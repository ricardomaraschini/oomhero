use clap::Parser;
use duration_str;
use log::info;
use log::warn;
use moka::sync::Cache;
use nix::sys::signal;
use oomhero::daemons;
use oomhero::events;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;
use std::env;
use std::process;
use std::str::FromStr;
use std::sync;
use std::thread;
use std::time;

#[derive(Parser)]
struct Arguments {
    #[arg(long, default_value = "75", help = "Warning memory usage watermark")]
    warning: i32,

    #[arg(long, default_value = "90", help = "Critical memory usage watermark")]
    critical: i32,

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
}

const COMMIT_DATE: &str = env!("VERGEN_GIT_COMMIT_DATE");
const COMMIT_HASH: &str = env!("VERGEN_GIT_SHA");
const COMMIT_DIRTY: &str = env!("VERGEN_GIT_DIRTY");

// parse_duration is used to parse the interval command line flag.
fn parse_duration(s: &str) -> Result<time::Duration, String> {
    duration_str::parse(s).map_err(|e| e.to_string())
}

// environment_overwrites overwrites command line arguments with environment variables.
// the use of environment variables will be deprecated, keeping them around for backwards
// compatibility.
fn environment_overwrites(args: &mut Arguments) {
    if let Ok(val) = env::var("WARNING") {
        warn!("WARNING env var is being deprecated (use flag)");
        args.warning = val.parse().expect("parsing env var");
    }

    if let Ok(val) = env::var("CRITICAL") {
        warn!("CRITICAL env var is being deprecated (use flag)");
        args.critical = val.parse().expect("parsing env var");
    }

    if let Ok(val) = env::var("COOLDOWN") {
        warn!("COOLDOWN env var is being deprecated (use flag)");
        args.cooldown_interval = parse_duration(&val).expect("parsing env var");
    }

    if let Ok(val) = env::var("WARNING_SIGNAL") {
        warn!("WARNING_SIGNAL env var is being deprecated (use flag)");
        args.warning_signal = signal::Signal::from_str(&val).expect("parsing env var")
    }

    if let Ok(val) = env::var("CRITICAL_SIGNAL") {
        warn!("CRITICAL_SIGNAL env var is being deprecated (use flag)");
        args.critical_signal = signal::Signal::from_str(&val).expect("parsing env var")
    }
}

fn main() {
    if env::var("RUST_LOG").is_err() {
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    env_logger::init();
    let mut args = Arguments::parse();
    if args.version {
        banner();
        return;
    }

    environment_overwrites(&mut args);
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
        let monitor = daemons::Monitor::new(&tx, args.warning as f32, args.critical as f32)
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
    info!("┌─┐┌─┐┌┬┐┬ ┬┌─┐┬─┐┌─┐       ");
    info!("│ȱ├│ȱ││││├─┤├┤ ├┬┘│ │       ");
    info!("└─┘└─┘┴ ┴┴ ┴└─┘┴└─└─┘'      ");
    info!(" ────                       ");
    info!("compile information:        ");
    info!("  commit_date:       {}     ", COMMIT_DATE);
    info!("  commit_hash:       {}     ", hash);
    info!("  dirty:             {}     ", COMMIT_DIRTY);
}

// active config prints the active configuration present int he arguments.
fn active_config(args: &Arguments) {
    info!("                            ");
    info!("active config:              ");
    info!("  warning:           {}%    ", args.warning);
    info!("  critical:          {}%    ", args.critical);
    info!("  loop_interval:     {:?}   ", args.loop_interval);
    info!("  cooldown_interval: {:?}   ", args.cooldown_interval);
    info!("  warning_signal:    {:?}   ", args.warning_signal);
    info!("  critical_signal:   {:?}   ", args.critical_signal);
}
