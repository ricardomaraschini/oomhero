use super::errors::Error;
use super::events;
use super::processes;
use moka::sync::Cache;
use nix::sys::signal;
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
// something to be used unbounded. signal_interval_secs governs how often we can send the same
// signal towards the same pid while last_signals keeps track of previously send signals. we limit
// the historic data to 1_000 different pids, we do not expect this to ever go beyound this.
pub struct Monitor<'a> {
    sink: &'a events::Transmitter,
    warning: f32,
    critical: f32,
    loop_interval: time::Duration,
    last_signals: Cache<i32, SignalRecord>,
    signal_interval: time::Duration,
}

impl<'a> Monitor<'a> {
    // new returns a new cgroups monitor. sink is used to send all events, warning and critical are
    // used to assess the memory usage while last_signals is used to keep track when was the last
    // time we signaled a process.
    pub fn new(
        sink: &'a events::Transmitter,
        warning: f32,
        critical: f32,
        loop_interval: time::Duration,
        signal_interval: time::Duration,
    ) -> Self {
        Monitor {
            sink,
            warning,
            critical,
            loop_interval,
            signal_interval,
            last_signals: Cache::new(1_000),
        }
    }

    // run starts the process monitor. this function lists as processes on the system and evalutes
    // their memory usage. signals are sent to the processes crossing the warning and critical
    // watermarks. it is important for the caller to constantly read from the sender passed in as
    // errors are send as messages through it. this function never returns. also important to know that
    // this function has just a very small sleep between interactions so when running this on a cluster
    // you better set proper resource.limits.cpu values as that is what guides how often we run the
    // loops.
    pub fn run(&self) {
        let mut last_pass = time::Instant::now();
        let mut passes: i64 = 0;
        loop {
            thread::sleep(self.loop_interval);

            let pids = match processes::list() {
                Ok(pids) => pids,
                Err(err) => {
                    self.sink.send(
                        events::Event::default()
                            .with_priority(events::Priority::High)
                            .with_message(format!("error listing processes: {err}")),
                    );
                    continue;
                }
            };

            for pid in pids {
                // skip pause container.
                if pid == 1 {
                    continue;
                }

                let usage = match self.assess_process_usage(pid) {
                    Ok(usage) => usage,
                    Err(err) => {
                        self.sink.send(
                            events::Event::default()
                                .with_pid(pid)
                                .with_priority(events::Priority::Low)
                                .with_message(format!("error reading memory usage: {err}")),
                        );
                        continue;
                    }
                };

                if usage >= self.critical {
                    self.send_signal(pid, signal::SIGUSR2, usage, "critical");
                    continue;
                }
                if usage >= self.warning {
                    self.send_signal(pid, signal::SIGUSR1, usage, "warning");
                    continue;
                }

                self.sink.send(
                    events::Event::default()
                        .with_pid(pid)
                        .with_priority(events::Priority::Low)
                        .with_memory_usage(usage)
                        .with_message(format!("memory usage within limits")),
                );
            }

            passes += 1;
            let since_last_pass = time::Instant::now() - last_pass;
            if since_last_pass.as_secs() < 60 {
                continue;
            }

            last_pass = time::Instant::now();
            let per_second = passes / since_last_pass.as_secs() as i64;
            self.sink.send(
                events::Event::default()
                    .with_priority(events::Priority::Low)
                    .with_message(format!("scans per second: {per_second}")),
            );
        }
    }
    // assess_process_usage reads the memory usage for the process and the limit, returns the usage
    // expressed as percent of the total utilization. if the pid has no max limit for memory usage
    // this function returns 0% as usage.
    fn assess_process_usage(&self, pid: i32) -> Result<f32, Error> {
        let has_limit = processes::has_memory_limit(pid)?;
        if has_limit == false {
            return Ok(0.0);
        }
        let mem_stats = processes::memory_stats(pid)?;
        let cur = mem_stats.0 as f32;
        let max = mem_stats.1 as f32;
        let usage: f32 = cur / max * 100.;
        Ok(usage)
    }

    // send_signal sends the provided signal to the provided process. this function also generate an
    // event either in case of success or failure. This function also accepts a description for the
    // signal (that is used only for giving the resultin event more context).
    fn send_signal(&self, pid: i32, sig: signal::Signal, usage: f32, desc: &str) {
        if let Some(last_signal) = self.last_signals.get(&pid) {
            if last_signal.kind == sig {
                let elapsed = time::Instant::now() - last_signal.when;
                if elapsed < self.signal_interval {
                    return;
                }
            }
        }

        if let Err(err) = processes::send_signal(pid, sig) {
            self.sink.send(
                events::Event::default()
                    .with_priority(events::Priority::High)
                    .with_message(format!("fail sending {desc} signal: {err}"))
                    .with_memory_usage(usage)
                    .with_pid(pid),
            );
            return;
        }

        self.last_signals.insert(
            pid,
            SignalRecord {
                when: time::Instant::now(),
                kind: sig,
            },
        );

        self.sink.send(
            events::Event::default()
                .with_priority(events::Priority::High)
                .with_message(format!("{desc} signal sent successfully"))
                .with_memory_usage(usage)
                .with_pid(pid),
        );
    }
}
