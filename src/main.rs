use clap::Parser;
use log::error;
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

const COMMIT_HASH: &str = env!("VERGEN_GIT_SHA");
const COMMIT_DIRTY: &str = env!("VERGEN_GIT_DIRTY");

#[tokio::main]
async fn main() {
    let environment = env_logger::Env::new().default_filter_or("info");
    env_logger::Builder::from_env(environment).init();

    let flags = arguments::Flags::parse();
    if flags.version {
        println!("{}", full_version());
        return;
    }

    if let Err(err) = flags.thresholds.validate() {
        warn!("{}", err);
        process::exit(1);
    }

    banner();
    info!("config: {}", &flags);

    let (stop_transmitter, stop_receiver) = sync::mpsc::sync_channel::<bool>(0);
    tokio::spawn(signal_handler(stop_transmitter));

    let (evt_transmitter, evt_receiver) = sync::mpsc::channel::<events::Event>();
    tokio::spawn(start_event_logger(evt_receiver));

    let tx = events::Transmitter::new(evt_transmitter);
    let syscgroups = system::SystemCGroups::default();
    let processes_explorer = processes::ProcFsReader::new(syscgroups);
    let signal_sender = signals::SignalSender::default();
    let monitor = daemons::Monitor::new(tx, flags.thresholds, processes_explorer, signal_sender)
        .with_cooldown_interval(flags.cooldown_interval)
        .with_loop_interval(flags.loop_interval)
        .with_warning_signal(flags.warning_signal)
        .with_critical_signal(flags.critical_signal);
    monitor.run(stop_receiver);
}

// signal_handler installs a signal handler for interrupt and terminate. Once one of these signals
// is received a `bool` message is sent through the provided notifier and then the async finishes.
// if this function fails to create the channel then a message is logged, nothing more is done.
async fn signal_handler(notifier: sync::mpsc::SyncSender<bool>) {
    let mut incoming_signals = match Signals::new([SIGINT, SIGTERM]) {
        Err(error) => {
            error!("unable to hook signal handler: {:?}", error);
            return;
        }
        Ok(channel) => channel,
    };
    incoming_signals.wait();
    info!("signal received, stopping daemon.");
    _ = notifier.send(true);
}

// start_event_logger spawns a new thread that keeps reading events from the provided channel
// until the process exist. This function just spawns the thread and immediatly returns.
async fn start_event_logger(rx: sync::mpsc::Receiver<events::Event>) {
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
}

// full_version returns the value of the cargo package version with a suffix of the commit hash
// used to build the binary. If uncommited changes are found then a -dirty string is added to
// the resulting version.
fn full_version() -> String {
    let hash = &COMMIT_HASH.to_string()[0..6];
    let mut dirty = "";
    if COMMIT_DIRTY == "true" {
        dirty = "-dirty";
    }
    format!("v{}-{}{}", env!("CARGO_PKG_VERSION"), hash, dirty)
}

// banner prints the banner.
fn banner() {
    info!("                              ");
    info!("    ,.   (   .      )        .");
    info!("   ('     )  )'     ,'        ");
    info!(" .; )  '▌(( (' )    ;(,  ,' ((");
    info!(" ▛▌▛▌▛▛▌▛▌█▌▛▘▛▌(..,( . )_  _'");
    info!(" ▙▌▙▌▌▌▌▌▌▙▖▌ ▙▌              ");
    info!("                 {}           ", full_version());
    info!("                              ");
}
