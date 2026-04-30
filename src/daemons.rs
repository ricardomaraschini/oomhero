use super::events;
use super::processes;
use super::thresholds;
use moka::sync::Cache;
use nix::sys::signal;
use std::process;
use std::thread;
use std::time;

// SignalRecord is a struct used to keep track of sent signals.
#[derive(Debug, Clone)]
struct SignalRecord {
    when: time::Instant,
    kind: signal::Signal,
}

// Monitor implements a daemon that monitors processes memory utilization on a system. The monitor
// is just a loop that never returns and can be started by calling run(). Important to note that
// the daemon is very cpu intensive. The idea is that one would govern how fast it evaluates
// memory usage by properly setting cpu limits on the pod where it is running. It is, by no means,
// something to be used unbounded. cooldown_interval_secs governs how often we can send the same
// signal towards the same pid while last_signals keeps track of previously sent signals. We limit
// the historic data to 1_000 different pids, we do not expect this to ever go beyond this.
pub struct Monitor<'a> {
    sink: &'a events::Transmitter,
    thresholds: &'a thresholds::UserProvided,
    processes_discover: &'a dyn processes::ProcessProvider,
    loop_interval: time::Duration,
    last_signals: Cache<i32, SignalRecord>,
    warning_signal: signal::Signal,
    critical_signal: signal::Signal,
    cooldown_interval: time::Duration,
}

impl<'a> Monitor<'a> {
    // new returns a new cgroups monitor. Sink is used to send all events, warning and critical are
    // used to assess the memory usage while last_signals is used to keep track when was the last
    // time we signaled a process.
    pub fn new(
        sink: &'a events::Transmitter,
        thresholds: &'a thresholds::UserProvided,
        processes_discover: &'a impl processes::ProcessProvider,
    ) -> Self {
        Monitor {
            sink,
            thresholds,
            processes_discover,
            loop_interval: time::Duration::from_millis(100),
            last_signals: Cache::new(1_000),
            warning_signal: signal::Signal::SIGUSR1,
            critical_signal: signal::Signal::SIGUSR2,
            cooldown_interval: time::Duration::from_secs(30),
        }
    }

    // with_loop_interval sets the interval used between the loops (the time between processes
    // scans).
    pub fn with_loop_interval(mut self, loop_interval: time::Duration) -> Self {
        self.loop_interval = loop_interval;
        self
    }

    // with_cooldown_interval sets the minimum time between sends of the same signal.
    pub fn with_cooldown_interval(mut self, cooldown_interval: time::Duration) -> Self {
        self.cooldown_interval = cooldown_interval;
        self
    }

    // with_warning_signal sets the signal to be sent upon warning threshold cross.
    pub fn with_warning_signal(mut self, warning_signal: signal::Signal) -> Self {
        self.warning_signal = warning_signal;
        self
    }

    // with_critical_signal sets the signal to be sent when a process crosses the critical
    // watermark.
    pub fn with_critical_signal(mut self, critical_signal: signal::Signal) -> Self {
        self.critical_signal = critical_signal;
        self
    }

    // run starts the process monitor. This function lists all processes on the system and evaluates
    // their memory and pressure data. Signals are sent to the processes crossing the warning and
    // critical watermarks. It is important for the caller to constantly read from the sender
    // passed in as errors are sent as messages through it. This function never returns. Also
    // important to know that this function has just a very small sleep between interactions so
    // when running this on a cluster you better set proper resource.limits.cpu values as that is
    // what guides how often we run the loops.
    pub fn run(&self) {
        let oomhero_pid = process::id() as i32;
        let mut last_pass = time::Instant::now();
        let mut passes: i64 = 0;
        loop {
            thread::sleep(self.loop_interval);

            let processes = match self.processes_discover.list() {
                Ok(processes) => processes,
                Err(err) => {
                    self.sink.send(
                        events::Event::high_prio()
                            .with_message(format!("error listing processes: {err}")),
                    );
                    continue;
                }
            };

            for process in processes {
                // skip pause container and our own process.
                if process.pid == 1 || process.pid == oomhero_pid {
                    continue;
                }

                let cd = match self.processes_discover.collect_process_data(process.pid) {
                    Ok(usage) => usage,
                    Err(err) => {
                        self.sink.send(
                            events::Event::low_prio()
                                .with_process(&process)
                                .with_message(format!("error collecting process data: {err}")),
                        );
                        continue;
                    }
                };

                let (warning, critical) = self.thresholds.check_against(&cd);
                if warning || critical {
                    if critical {
                        self.send_signal(&process, self.critical_signal, &cd, "critical");
                    } else {
                        self.send_signal(&process, self.warning_signal, &cd, "warning");
                    }
                    continue;
                }

                self.sink.send(
                    events::Event::low_prio()
                        .with_process_collected_data(&process, &cd)
                        .with_message(format!("process usage within limits")),
                );
            }

            passes += 1;
            let since_last_pass = time::Instant::now() - last_pass;
            if since_last_pass.as_secs() < 60 {
                continue;
            }

            let per_second = passes / since_last_pass.as_secs() as i64;
            self.sink.send(
                events::Event::low_prio().with_message(format!("scans per second: {per_second}")),
            );

            last_pass = time::Instant::now();
            passes = 0;
        }
    }

    // send_signal sends the provided signal to the provided process. This function also generates an
    // event either in case of success or failure. This function also accepts a description for the
    // signal (that is used only for giving the resulting event more context).
    fn send_signal(
        &self,
        process: &processes::Process,
        sig: signal::Signal,
        cd: &processes::CollectedData,
        desc: &str,
    ) {
        if let Some(last_signal) = self.last_signals.get(&process.pid) {
            if last_signal.kind == sig {
                let elapsed = time::Instant::now() - last_signal.when;
                if elapsed < self.cooldown_interval {
                    return;
                }
            }
        }

        if let Err(err) = self.processes_discover.send_signal(process.pid, sig) {
            self.sink.send(
                events::Event::high_prio()
                    .with_process_collected_data(process, &cd)
                    .with_message(format!("fail sending {desc} signal: {err}")),
            );
            return;
        }

        self.last_signals.insert(
            process.pid,
            SignalRecord {
                when: time::Instant::now(),
                kind: sig,
            },
        );

        self.sink.send(
            events::Event::high_prio()
                .with_process_collected_data(process, &cd)
                .with_message(format!("{desc} signal sent successfully")),
        );
    }
}
