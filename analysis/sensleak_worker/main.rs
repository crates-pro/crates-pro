use std::thread;
use std::time::Duration;

fn main() {
    println!("Starting the program. Press Ctrl+C to stop.");
    loop {
        println!("sensleak_worker");
        thread::sleep(Duration::from_secs(1));
    }
}
