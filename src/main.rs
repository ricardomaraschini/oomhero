use clap::Parser;
use duration_str;
use log::info;
use log::trace;
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

#[derive(Parser, Debug)]
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
    info!("{:?}", &args);

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
        trace!("{:?}", &event);
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
    let hash = &COMMIT_HASH.to_string()[0..6];
    let mut version = format!("{}-{}", COMMIT_DATE, hash);
    if COMMIT_DIRTY == "true" {
        version = format!("{}-dirty", version);
    }

    info!("                              ");
    info!("    ,.   (   .      )        .");
    info!("   ('     )  )'     ,'        ");
    info!(" .; )  '▌(( (' )    ;(,  ,' ((");
    info!(" ▛▌▛▌▛▛▌▛▌█▌▛▘▛▌(..,( . )_  _'");
    info!(" ▙▌▙▌▌▌▌▌▌▙▖▌ ▙▌              ");
    info!("                v{}           ", version);
    info!("                              ");
}
