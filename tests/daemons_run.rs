use mockall::predicate;
use nix::sys::signal;
use oomhero::arguments;
use oomhero::daemons;
use oomhero::errors;
use oomhero::events;
use oomhero::processes;
use oomhero::signals;
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time;

#[test]
fn daemons_run_processes_cooldown_enforcement() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(|_evt| {});

    let thresholds = arguments::Thresholds {
        memory_usage_warning: 70,
        memory_usage_critical: 90,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();
    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 95.,
                ..Default::default()
            })
        });

    let mut signals_sender = signals::MockSender::new();
    // we expect exactly one signal to be sent, despite the loop running multiple times.
    signals_sender
        .expect_send()
        .times(1)
        .returning(move |sig, pid| {
            assert_eq!(sig, signal::SIGKILL);
            assert_eq!(pid, 2);
            Ok(())
        });

    thread::spawn(move || {
        let monitor =
            daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
                .with_loop_interval(time::Duration::from_millis(10))
                .with_critical_signal(signal::SIGKILL)
                .with_cooldown_interval(time::Duration::from_secs(60));
        monitor.run(rx);
    });

    // wait for the monitor to a few times and then stop it.
    std::thread::sleep(time::Duration::from_millis(100));
    _ = tx.send(true);

    Ok(())
}

#[test]
fn daemons_run_processes_exclusion() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();
    let oomhero_pid = process::id() as i32;

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(evt.pid, 2);
        assert_eq!(evt.message, String::from("process usage within limits"));
        _ = tx.send(true);
    });

    let thresholds = arguments::Thresholds {
        memory_usage_warning: 70,
        memory_usage_critical: 90,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(move || {
        Ok(vec![
            processes::Process {
                pid: 1,
                cmdline: String::from("init"),
            },
            processes::Process {
                pid: oomhero_pid,
                cmdline: String::from("oomhero"),
            },
            processes::Process {
                pid: 2,
                cmdline: String::from("monitored_process"),
            },
        ])
    });

    // We only expect collect_process_data for PID 2.
    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 10.,
                oom_score: 0,
                pressure: processes::Pressure::default(),
            })
        });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(1))
        .times(0);
    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(oomhero_pid))
        .times(0);

    let signals_sender = signals::MockSender::new();

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_cpu_pressure_critical() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("critical signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        cpu_pressure_warning: 10,
        cpu_pressure_critical: 20,
        stall_severity: arguments::StallSeverity::Some,
        stall_window: arguments::StallWindow::Avg10,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                pressure: processes::Pressure {
                    cpu: processes::PressureData {
                        some: processes::PressureAverages {
                            avg10: 21.,
                            avg60: 0.,
                            avg300: 0.,
                            total: 0.,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGUSR2);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_critical_but_only_pressure_thresholds() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(evt.message, String::from("process usage within limits"));
        assert_eq!(evt.pid, 2);
        _ = tx.send(true);
    });

    let thresholds = arguments::Thresholds {
        memory_pressure_warning: 70,
        memory_pressure_critical: 90,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 91.,
                oom_score: 0,
                pressure: processes::Pressure {
                    ..Default::default()
                },
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().never();

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_critical() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("critical signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        memory_usage_warning: 70,
        memory_usage_critical: 90,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 91.,
                oom_score: 0,
                pressure: processes::Pressure {
                    ..Default::default()
                },
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGKILL);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_critical_signal(signal::SIGKILL)
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_io_pressure_critical() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("critical signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        io_pressure_warning: 10,
        io_pressure_critical: 20,
        stall_severity: arguments::StallSeverity::Full,
        stall_window: arguments::StallWindow::Avg10,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                pressure: processes::Pressure {
                    io: processes::PressureData {
                        full: processes::PressureAverages {
                            avg10: 21.,
                            avg60: 0.,
                            avg300: 0.,
                            total: 0.,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGABRT);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_critical_signal(signal::SIGABRT)
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_memory_pressure_critical() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("critical signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        memory_pressure_warning: 10,
        memory_pressure_critical: 20,
        stall_severity: arguments::StallSeverity::Full,
        stall_window: arguments::StallWindow::Avg10,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 50.,
                oom_score: 0,
                pressure: processes::Pressure {
                    memory: processes::PressureData {
                        full: processes::PressureAverages {
                            avg10: 21.,
                            avg60: 0.,
                            avg300: 0.,
                            total: 0.,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGKILL);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_critical_signal(signal::SIGKILL)
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_mixed_thresholds_critical_precedence() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("critical signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        memory_usage_warning: 70,
        memory_usage_critical: 90,
        memory_pressure_warning: 10,
        memory_pressure_critical: 20,
        stall_severity: arguments::StallSeverity::Some,
        stall_window: arguments::StallWindow::Avg10,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 75., // Warning level for memory usage
                oom_score: 0,
                pressure: processes::Pressure {
                    memory: processes::PressureData {
                        some: processes::PressureAverages {
                            avg10: 25., // Critical level for memory pressure
                            avg60: 0.,
                            avg300: 0.,
                            total: 0.,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGKILL);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_critical_signal(signal::SIGKILL)
            .with_warning_signal(signal::SIGUSR1)
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_with_warning() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(
            evt.message,
            String::from("warning signal sent successfully")
        );
    });

    let thresholds = arguments::Thresholds {
        memory_usage_warning: 70,
        memory_usage_critical: 90,
        ..Default::default()
    };

    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 75.,
                oom_score: 0,
                pressure: processes::Pressure {
                    ..Default::default()
                },
            })
        });

    let mut signals_sender = signals::MockSender::new();
    signals_sender.expect_send().returning(move |sig, pid| {
        assert_eq!(sig, signal::SIGUSR1);
        assert_eq!(pid, 2);
        _ = tx.send(true);
        Ok(())
    });

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_processes_within_limits() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().returning(move |evt| {
        assert_eq!(evt.message, String::from("process usage within limits"));
        if evt.pid == 3 {
            _ = tx.send(true);
        }
    });

    let thresholds = arguments::Thresholds::default();
    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![
            processes::Process {
                pid: 2,
                cmdline: String::from("command_line_pid_2"),
            },
            processes::Process {
                pid: 3,
                cmdline: String::from("command_line_pid_3"),
            },
        ])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 10.,
                oom_score: 0,
                pressure: processes::Pressure {
                    ..Default::default()
                },
            })
        });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(3))
        .returning(|_pid| {
            Ok(processes::CollectedData {
                memory_max: 100.,
                memory_current: 20.,
                oom_score: 0,
                pressure: processes::Pressure {
                    ..Default::default()
                },
            })
        });

    let signals_sender = signals::MockSender::new();

    let monitor =
        daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
            .with_loop_interval(time::Duration::from_millis(50))
            .with_cooldown_interval(time::Duration::from_mins(1));
    monitor.run(rx);
    Ok(())
}

#[test]
fn daemons_run_fail_collecting_process_data() -> Result<(), errors::Error> {
    let (tx, rx) = mpsc::channel::<bool>();

    let mut events_sink = events::MockSender::new();
    events_sink.expect_send().times(0);

    let thresholds = arguments::Thresholds::default();
    let mut processes_explorer = processes::MockProcessProvider::new();

    processes_explorer.expect_list().returning(|| {
        Ok(vec![processes::Process {
            pid: 2,
            cmdline: String::from("command_line_pid_2"),
        }])
    });

    processes_explorer
        .expect_collect_process_data()
        .with(predicate::eq(2))
        .returning(|_pid| Err(errors::Error::Message(String::from("failed"))));

    let signals_sender = signals::MockSender::new();

    thread::spawn(move || {
        let monitor =
            daemons::Monitor::new(events_sink, thresholds, processes_explorer, signals_sender)
                .with_loop_interval(time::Duration::from_millis(50))
                .with_cooldown_interval(time::Duration::from_mins(1));
        monitor.run(rx);
    });

    // wait for the monitor to a few times and then stop it.
    std::thread::sleep(time::Duration::from_millis(100));
    _ = tx.send(true);
    Ok(())
}
