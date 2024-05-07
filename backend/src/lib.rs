pub mod quary_server;

use crate::quary_server::Server;
use std::{error::Error, sync::Arc};
use tokio::signal;
use tokio::time::{self, Duration};

const SLEEP_TIME_SECS: Option<u64> = Some(20); // The opening time for server

/// restful exposed to frontend
#[tokio::main]
pub async fn run_quary_server() -> Result<(), Box<dyn Error>> {
    let server = Arc::new(Server::new());

    let server_clone = Arc::clone(&server);

    // start
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // wait for time or infinitely
    match SLEEP_TIME_SECS {
        Some(secs) => {
            let sleep_duration = Duration::from_secs(secs);
            tokio::select! {
                _ = time::sleep(sleep_duration) => {
                    println!("Time to close the server after waiting for {} seconds.", secs);
                }
                _ = signal::ctrl_c() => {
                    println!("Received Ctrl+C - shutting down.");
                }
            }
        }
        None => {
            println!("No timeout set; the server will run indefinitely. Press Ctrl+C to stop.");
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        }
    }

    // close the server
    if let Err(e) = server.close().await {
        eprintln!("Failed to close the server gracefully: {}", e);
    } else {
        println!("Server closed gracefully.");
    }

    Ok(())
}
