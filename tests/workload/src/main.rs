use std::thread;
use std::time;

fn main() {
    loop {
        println!("ping");
        thread::sleep(time::Duration::from_secs(1));
    }
}
