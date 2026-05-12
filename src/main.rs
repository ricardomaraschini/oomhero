use clap::Parser;
use log::info;
use log::trace;
use log::warn;
use moka::sync::Cache;
use oomhero::arguments;
use oomhero::daemons;
use oomhero::events;
use oomhero::processes;
use oomhero::signals;
use oomhero::system;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;
use std::env;
use std::process;
use std::sync;
use std::thread;

const COMMIT_HASH: &str = env!("VERGEN_GIT_SHA");
const COMMIT_DIRTY: &str = env!("VERGEN_GIT_DIRTY");

fn main() {
    let environment = env_logger::Env::new().default_filter_or("info");
    env_logger::Builder::from_env(environment).init();

    let flags = arguments::Flags::parse();
    if flags.version {
        banner();
        return;
    }

    if let Err(err) = flags.thresholds.validate() {
        warn!("{}", err);
        process::exit(1);
    }

    banner();
    info!("config: {}", &flags);

    let mut incoming_signals =
        Signals::new([SIGINT, SIGTERM]).expect("failed to setup signal handlers");

    let (stop_tx, stop_rx) = sync::mpsc::sync_channel::<bool>(0);

    thread::spawn(move || {
        incoming_signals.wait();
        info!("signal received, stopping daemon.");
        _ = stop_tx.send(true);
    });

    let (tx, rx) = sync::mpsc::channel::<events::Event>();

    start_event_logger(rx);

    let tx = events::Transmitter::new(tx);
    let syscgroups = system::SystemCGroups::default();
    let processes_explorer = processes::ProcFsReader::new(syscgroups);
    let signal_sender = signals::SignalSender::default();
    let monitor = daemons::Monitor::new(tx, flags.thresholds, processes_explorer, signal_sender)
        .with_cooldown_interval(flags.cooldown_interval)
        .with_loop_interval(flags.loop_interval)
        .with_warning_signal(flags.warning_signal)
        .with_critical_signal(flags.critical_signal);
    monitor.run(stop_rx);
}

// start_event_logger spawns a new thread that keeps reading events from the provided channel
// until the process exist. This function just spawns the thread and immediatly returns.
fn start_event_logger(rx: sync::mpsc::Receiver<events::Event>) {
    thread::spawn(move || {
        let last_messages: Cache<i32, events::Event> = Cache::new(1_000);
        for event in rx {
            trace!("{:?}", &event);
            if let events::Priority::High = event.priority {
                last_messages.insert(event.pid, event.clone());
                warn!("{}", event);
                continue;
            }
            if let Some(previous_event) = last_messages.get(&event.pid)
                && !event.deviates_significantly(&previous_event)
            {
                continue;
            }

            last_messages.insert(event.pid, event.clone());
            info!("{}", event);
        }
    });
}

// banner prints the banner.
fn banner() {
    let version = env!("CARGO_PKG_VERSION");
    let hash = &COMMIT_HASH.to_string()[0..6];
    let mut dirty = "";
    if COMMIT_DIRTY == "true" {
        dirty = "-dirty";
    }
    info!("                              ");
    info!("    ,.   (   .      )        .");
    info!("   ('     )  )'     ,'        ");
    info!(" .; )  '▌(( (' )    ;(,  ,' ((");
    info!(" ▛▌▛▌▛▛▌▛▌█▌▛▘▛▌(..,( . )_  _'");
    info!(" ▙▌▙▌▌▌▌▌▌▙▖▌ ▙▌              ");
    info!("                 v{}-{}{}     ", version, hash, dirty);
    info!("                              ");
}
