//! Integration test for our server
//!

extern crate database;
use database::quary_server;
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;

#[tokio::test]
async fn test_server_responses() {
    let (tx, _rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let server = Arc::new(quary_server::Server::new());
        server
            .start()
            .await
            .unwrap_or_else(|e| eprintln!("Server error: {}", e));
    });

    tokio::time::sleep(Duration::from_secs(2)).await;

    let client = reqwest::Client::new();


    let resp = client
        .get("http://localhost:3000/crates/test_crate")
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    println!("{}", resp.text().await.unwrap());

    let _ = tx.send(());
}
