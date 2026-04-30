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
    #[arg(long, default_value = "100ms", help = "How often scan all processes", value_parser = parse_duration)]
    loop_interval: time::Duration,

    #[arg(long, default_value = "30s", help = "Interval between signals", value_parser = parse_duration)]
    cooldown_interval: time::Duration,

    #[arg(long, default_value = "SIGUSR1", help = "Signal send on warning")]
    warning_signal: signal::Signal,

    #[arg(long, default_value = "SIGUSR2", help = "Signal send on critical")]
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
    info!("compile information:          ");
    info!(" commit_date:       {}        ", COMMIT_DATE);
    info!(" commit_hash:       {}        ", hash);
    info!(" dirty:             {}        ", COMMIT_DIRTY);
}

// active config prints the active configuration present int he arguments.
fn active_config(args: &Arguments) {
    let t = &args.thresholds;
    info!("                              ");
    info!("active config:                ");
    info!(" memory_usage_warning:    {}% ", t.memory_usage_warning);
    info!(" memory_usage_critical:   {}% ", t.memory_usage_critical);
    info!(" memory_pressure_warning: {}% ", t.memory_pressure_warning);
    info!(" memory_pressure_critical:{}% ", t.memory_pressure_critical);
    info!(" io_pressure_warning:     {}% ", t.io_pressure_warning);
    info!(" io_pressure_critical:    {}% ", t.io_pressure_critical);
    info!(" cpu_pressure_warning:    {}% ", t.cpu_pressure_warning);
    info!(" cpu_pressure_critical    {}% ", t.cpu_pressure_critical);
    info!(" loop_interval:           {:?}", args.loop_interval);
    info!(" cooldown_interval:       {:?}", args.cooldown_interval);
    info!(" warning_signal:          {:?}", args.warning_signal);
    info!(" critical_signal:         {:?}", args.critical_signal);
    info!("                              ");

    let memusage = t.has_memory_usage_threholds();
    let mempress = t.has_memory_pressure_thresholds();
    let iopress = t.has_io_pressure_thresholds();
    let cpupress = t.has_cpu_pressure_thresholds();
    info!("enabled checks:               ");
    info!(" memory_usage:            {}  ", memusage);
    info!(" memory_pressure:         {}  ", mempress);
    info!(" io_pressure:             {}  ", iopress);
    info!(" cpu_pressure:            {}  ", cpupress);
    info!("                              ");
}
