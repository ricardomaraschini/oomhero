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

// Monitor implements a daemon that monitor processes memory utilization on a system. the monitor
// is just a loop that never returns and can be started by calling run(). important to note that
// the daemon is very cpu intensive. the idea is that one would govern how fast it evaluates
// memory usage by properly setting cpu limits on the pod where it is running. it is, by no means,
// something to be used unbounded. cooldown_interval_secs governs how often we can send the same
// signal towards the same pid while last_signals keeps track of previously send signals. we limit
// the historic data to 1_000 different pids, we do not expect this to ever go beyound this.
pub struct Monitor<'a> {
    sink: &'a events::Transmitter,
    thresholds: &'a thresholds::UserProvided,
    loop_interval: time::Duration,
    last_signals: Cache<i32, SignalRecord>,
    warning_signal: signal::Signal,
    critical_signal: signal::Signal,
    cooldown_interval: time::Duration,
}

impl<'a> Monitor<'a> {
    // new returns a new cgroups monitor. sink is used to send all events, warning and critical are
    // used to assess the memory usage while last_signals is used to keep track when was the last
    // time we signaled a process.
    pub fn new(sink: &'a events::Transmitter, thresholds: &'a thresholds::UserProvided) -> Self {
        Monitor {
            sink,
            thresholds,
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

    // run starts the process monitor. this function lists as processes on the system and evalutes
    // their memory and pressure data. signals are sent to the processes crossing the warning and
    // critical watermarks. it is important for the caller to constantly read from the sender
    // passed in as errors are send as messages through it. this function never returns. also
    // important to know that this function has just a very small sleep between interactions so
    // when running this on a cluster you better set proper resource.limits.cpu values as that is
    // what guides how often we run the loops.
    pub fn run(&self) {
        let oomhero_pid = process::id() as i32;
        let mut last_pass = time::Instant::now();
        let mut passes: i64 = 0;
        loop {
            thread::sleep(self.loop_interval);

            let processes = match processes::list() {
                Ok(processes) => processes,
                Err(err) => {
                    self.sink.send(
                        events::Event::default()
                            .with_priority(events::Priority::High)
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

                let cd = match processes::collect_process_data(process.pid) {
                    Ok(usage) => usage,
                    Err(err) => {
                        self.sink.send(
                            events::Event::default()
                                .with_cmdline(process.cmdline)
                                .with_pid(process.pid)
                                .with_priority(events::Priority::Low)
                                .with_message(format!("error collecting process data: {err}")),
                        );
                        continue;
                    }
                };

                let (warning, critical) = self.thresholds.check_against(&cd);
                if critical {
                    self.send_signal(&process, self.critical_signal, &cd, "critical");
                    continue;
                } else if warning {
                    self.send_signal(&process, self.warning_signal, &cd, "warning");
                    continue;
                }

                self.sink.send(
                    events::Event::default()
                        .with_cmdline(process.cmdline)
                        .with_pid(process.pid)
                        .with_priority(events::Priority::Low)
                        .with_collected_data(&cd)
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
                events::Event::default()
                    .with_priority(events::Priority::Low)
                    .with_message(format!("scans per second: {per_second}")),
            );

            last_pass = time::Instant::now();
            passes = 0;
        }
    }

    // send_signal sends the provided signal to the provided process. this function also generate an
    // event either in case of success or failure. This function also accepts a description for the
    // signal (that is used only for giving the resultin event more context).
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

        if let Err(err) = processes::send_signal(process.pid, sig) {
            self.sink.send(
                events::Event::default()
                    .with_priority(events::Priority::High)
                    .with_pid(process.pid)
                    .with_cmdline(process.cmdline.clone())
                    .with_collected_data(&cd)
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
            events::Event::default()
                .with_priority(events::Priority::High)
                .with_pid(process.pid)
                .with_cmdline(process.cmdline.clone())
                .with_collected_data(&cd)
                .with_message(format!("{desc} signal sent successfully")),
        );
    }
}
