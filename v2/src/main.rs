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
use std::sync;
use std::thread;
use std::time;

#[derive(Parser)]
struct Arguments {
    #[arg(long, default_value = "75", help = "Warning memory usage watermark")]
    warning: i32,

    #[arg(long, default_value = "90", help = "Critical memory usage watermark")]
    critical: i32,

    #[arg(long, default_value = "10ms", help = "How often scan all processes", value_parser = parse_duration)]
    interval: time::Duration,

    #[arg(long, default_value = "30s", help = "Interval between signals", value_parser = parse_duration)]
    cooldown: time::Duration,

    #[arg(long, default_value = "SIGUSR1", help = "Signal send on warning")]
    warning_signal: signal::Signal,

    #[arg(long, default_value = "SIGUSR2", help = "Signal send on critical")]
    critical_signal: signal::Signal,
}

// parse_duration is used to parse the interval command line flag.
fn parse_duration(s: &str) -> Result<time::Duration, String> {
    duration_str::parse(s).map_err(|e| e.to_string())
}

// environment_overwrites overrides command line arguments with environment variables. the use of
// environment variables will be deprecated, keeping them around for backwards compatibility.
fn environment_overwrites(args: &mut Arguments) {
    if let Ok(warning) = env::var("WARNING") {
        warn!("WARNING environment variable is being deprecated, use --warning flag instead");
        args.warning = warning.parse().expect("failed to parse warning env");
    }

    if let Ok(critical) = env::var("CRITICAL") {
        warn!("CRITICAL environment variable is being deprecated, use --critical flag instead");
        args.critical = critical.parse().expect("failed to parse critical env");
    }

    if let Ok(cooldown) = env::var("COOLDOWN") {
        warn!("COOLDOWN environment variable is being deprecated, use --cooldown flag instead");
        args.cooldown = parse_duration(&cooldown).expect("failed to parse cooldown env");
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
    environment_overwrites(&mut args);
    print_welcome(&args);

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
        let monitor = daemons::Monitor::new(
            &tx,
            args.warning as f32,
            args.critical as f32,
            args.interval,
            args.cooldown,
            args.warning_signal,
            args.critical_signal,
        );
        monitor.run();
    });

    let last_messages: Cache<i32, events::Event> = Cache::new(1_000);
    for event in rx {
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

// print_welcome prints the welcome banner and all paramaters as parsed from the command line
// arguments or environment variables.
fn print_welcome(args: &Arguments) {
    info!(" /$$");
    info!("| $$");
    info!("| $$$$$$$   /$$$$$$   /$$$$$$   /$$$$$$ ");
    info!("| $$__  $$ /$$__  $$ /$$__  $$ /$$__  $$");
    info!("| $$  | $$| $$$$$$$$| $$  |__/| $$  | $$");
    info!("| $$  | $$| $$_____/| $$      | $$  | $$");
    info!("| $$  | $$|  $$$$$$$| $$      |  $$$$$$/");
    info!("|__/  |__/  |_______/|__/      |______/");
    info!("warning: {}", args.warning);
    info!("critical: {}", args.critical);
    info!("interval: {:?}", args.interval);
    info!("cooldown: {:?}", args.cooldown);
    info!("warning_signal: {:?}", args.warning_signal);
    info!("critical_signal: {:?}", args.critical_signal);
}
