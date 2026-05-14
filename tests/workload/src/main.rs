use axum::Router;
use axum::extract::State;
use axum::routing::get;
use signal_hook::consts::SIGUSR1;
use signal_hook::consts::SIGUSR2;
use signal_hook::iterator::Signals;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time;

#[derive(Clone)]
struct AppState {
    cpu_tx: mpsc::SyncSender<bool>,
    cpu_next: Arc<Mutex<bool>>,
    mem_tx: mpsc::SyncSender<bool>,
    mem_next: Arc<Mutex<bool>>,
}

#[tokio::main]
async fn main() {
    let (cpu_tx, cpu_rx) = mpsc::sync_channel::<bool>(1);
    thread::spawn(move || cpu_usage(cpu_rx));

    let (mem_tx, mem_rx) = mpsc::sync_channel::<bool>(1);
    thread::spawn(move || mem_usage(mem_rx));

    let state = AppState {
        cpu_tx: cpu_tx.clone(),
        cpu_next: Arc::new(Mutex::new(true)),
        mem_tx: mem_tx.clone(),
        mem_next: Arc::new(Mutex::new(true)),
    };

    let state_copy = state.clone();
    thread::spawn(move || signal_handler(state_copy));

    let router = Router::new()
        .route("/cpu", get(cpu_handler))
        .route("/mem", get(mem_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999")
        .await
        .expect("failed to start listening");

    axum::serve(listener, router)
        .await
        .expect("failed to start serving http endpoints");
}

// signal handler handles both SIGUSR1 and SIGUSR2. Upon receiving disables both memory and cpu
// consumption in the threads.
fn signal_handler(state: AppState) {
    let mut incoming_signals = match Signals::new([SIGUSR1, SIGUSR2]) {
        Err(error) => panic!("unable to hook signal handler: {:?}", error),
        Ok(channel) => channel,
    };

    for signal in incoming_signals.forever() {
        println!("signal {:?} received", signal);
        state.mem_tx.send(false).expect("failed to stop mem usage");
        *state.mem_next.lock().unwrap() = true;

        state.cpu_tx.send(false).expect("failed to stop cpu usage");
        *state.cpu_next.lock().unwrap() = true;
    }
}

// cpu_handler flips the cpu consumption on and off. One call turns it on, the next turns it off.
async fn cpu_handler(State(state): State<AppState>) -> &'static str {
    println!("cpu switch pressed");
    let mut next = state.cpu_next.lock().unwrap();
    state.cpu_tx.send(*next).expect("failed sending message");
    *next = !*next;
    "cpu ack"
}

// mem_handler flips the mem consumption on and off. One call turns it on, the next turns it off.
async fn mem_handler(State(state): State<AppState>) -> &'static str {
    println!("mem switch pressed");
    let mut next = state.mem_next.lock().unwrap();
    state.mem_tx.send(*next).expect("failed sending message");
    *next = !*next;
    "mem ack"
}

// mem_usage is a loop that may or may not increase the memory consumption in the workload process.
// If a true value is read from the Receiver then we start to consume 1MB of memory per second. If
// a false is read instead then we truncate the used memory to 0.
fn mem_usage(switch: mpsc::Receiver<bool>) {
    let mut previous: bool = false;
    let mut ballast: Vec<String> = Vec::new();
    let delay = time::Duration::from_secs(1);
    loop {
        let consume = match switch.try_recv() {
            Err(_) => previous,
            Ok(value) => value,
        };

        match consume {
            true => ballast.push("x".repeat(1_024 * 1_024)),
            false => ballast.truncate(0),
        }

        previous = consume;
        thread::sleep(delay);
    }
}

// cpu_usage is a loop with a default delay of 1 second between interactions. This can be sped up
// by sending a different speed through the Receiver, from which we are constantly reading.
fn cpu_usage(throttle: mpsc::Receiver<bool>) {
    let mut previous: bool = false;
    loop {
        let speed_up = match throttle.try_recv() {
            Err(_) => previous,
            Ok(value) => value,
        };

        match speed_up {
            false => thread::sleep(time::Duration::from_secs(1)),
            true => {
                for _ in 1..=1_000_000 {
                    _ = 2 + 2;
                }
            }
        }

        previous = speed_up;
    }
}
