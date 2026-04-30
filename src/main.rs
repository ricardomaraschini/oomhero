use clap::Parser;
use duration_str;
use log::info;
use log::warn;
use moka::sync::Cache;
use nix::sys::signal;
use oomhero::daemons;
use oomhero::events;
use oomhero::thresholds;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;
use std::env;
use std::process;
use std::sync;
use std::thread;
use std::time;

#[derive(Parser)]
struct Arguments {
    #[arg(
        long,
        default_value = "100ms",
        value_parser = parse_duration,
        help = "Interval to be used as a pause between process scans"
    )]
    loop_interval: time::Duration,

    #[arg(
        long,
        default_value = "30s",
        value_parser = parse_duration,
        help = "Interval to wait before sending the same signal to the same process"
    )]
    cooldown_interval: time::Duration,

    #[arg(
        long,
        default_value = "SIGUSR1",
        help = "Signal to be send when a process crosses the warning watermark"
    )]
    warning_signal: signal::Signal,

    #[arg(
        long,
        default_value = "SIGUSR2",
        help = "Signal to be send when a process crosses the critical watermark"
    )]
    critical_signal: signal::Signal,

    #[arg(long, default_value = "false", help = "Print version")]
    version: bool,

    #[arg(long, default_value = "false", help = "Set logging to verbose")]
    verbose: bool,

    #[command(flatten)]
    thresholds: thresholds::UserProvided,
}

const COMMIT_DATE: &str = env!("VERGEN_GIT_COMMIT_DATE");
const COMMIT_HASH: &str = env!("VERGEN_GIT_SHA");
const COMMIT_DIRTY: &str = env!("VERGEN_GIT_DIRTY");

// parse_duration is used to parse the interval command line flag.
fn parse_duration(s: &str) -> Result<time::Duration, String> {
    duration_str::parse(s).map_err(|e| e.to_string())
}

fn main() {
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    env_logger::init();
    let args = Arguments::parse();
    if args.version {
        banner();
        return;
    }

    if let Err(err) = args.thresholds.validate() {
        warn!("{:?}", err);
        process::exit(1);
    }

    banner();
    active_config(&args);

    let mut incoming_signals =
        Signals::new([SIGINT, SIGTERM]).expect("failed to setup signal handlers");

    thread::spawn(move || {
        incoming_signals.wait();
        info!("signal received, ending process.");
        process::exit(0);
    });

    let (tx, rx) = sync::mpsc::channel::<events::Event>();

    thread::spawn(move || {
        let tx = events::Transmitter::new(tx);
        let monitor = daemons::Monitor::new(&tx, &args.thresholds)
            .with_cooldown_interval(args.cooldown_interval)
            .with_loop_interval(args.loop_interval)
            .with_warning_signal(args.warning_signal)
            .with_critical_signal(args.critical_signal);
        monitor.run();
    });

    let last_messages: Cache<i32, events::Event> = Cache::new(1_000);
    for event in rx {
        if args.verbose {
            if let events::Priority::High = event.priority {
                warn!("{:?}", event);
                continue;
            }
            info!("{:?}", event);
            continue;
        }

        if let events::Priority::High = event.priority {
            last_messages.insert(event.pid, event.clone());
            warn!("{:?}", event);
            continue;
        }

        if let Some(previous_event) = last_messages.get(&event.pid) {
            if event.deviates_significantly(&previous_event) == false {
                continue;
            }
        }

        last_messages.insert(event.pid, event.clone());
        info!("{:?}", event);
    }
}

// banner prints the banner.
fn banner() {
    let hash = &COMMIT_HASH.to_string()[0..10];
    info!("┌─┐┌─┐┌┬┐┬ ┬┌─┐┬─┐┌─┐         ");
    info!("│ȱ├│ȱ││││├─┤├┤ ├┬┘│ │         ");
    info!("└─┘└─┘┴ ┴┴ ┴└─┘┴└─└─┘'        ");
    info!(" ────                         ");
    info!(" commit date:              {} ", COMMIT_DATE);
    info!(" commit hash:              {} ", hash);
    info!(" dirty:                    {} ", COMMIT_DIRTY);
}

// active config prints the active configuration present int he arguments.
fn active_config(args: &Arguments) {
    let t = &args.thresholds;
    let memusage = t.has_memory_usage_threholds();
    let mempress = t.has_memory_pressure_thresholds();
    let iopress = t.has_io_pressure_thresholds();
    let cpupress = t.has_cpu_pressure_thresholds();

    info!("");
    info!("general config:");
    info!(" loop interval:           {:?}", args.loop_interval);
    info!(" cooldown interval:       {:?}", args.cooldown_interval);
    info!(" warning signal:          {:?}", args.warning_signal);
    info!(" critical signal:         {:?}", args.critical_signal);
    if mempress || iopress || cpupress {
        info!(" stall severity:          {:?}", t.stall_severity);
        info!(" stall window:            {:?}", t.stall_window);
    }

    info!("");
    info!("enabled checks:");
    if memusage {
        info!(" memory usage:");
        info!("  memory usage_warning:    {}%", t.memory_usage_warning);
        info!("  memory usage_critical:   {}%", t.memory_usage_critical);
    }
    if mempress {
        info!(" memory pressure:");
        info!("  memory pressure warning: {}%", t.memory_pressure_warning);
        info!("  memory pressure critical:{}%", t.memory_pressure_critical);
    }
    if iopress {
        info!(" io pressure:");
        info!("  io pressure warning:     {}%", t.io_pressure_warning);
        info!("  io pressure critical:    {}%", t.io_pressure_critical);
    }
    if cpupress {
        info!(" cpu pressure:");
        info!("  cpu pressure warning:    {}%", t.cpu_pressure_warning);
        info!("  cpu pressure critical    {}%", t.cpu_pressure_critical);
    }
    info!("                              ");
}
