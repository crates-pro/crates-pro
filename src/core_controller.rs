//! the core controller module
//! It receives messages from Kafka MQ one by one,
//! parse them, and store it into tugraph, and notify
//! other processes.

use crate::cli::CratesProCli;

use repo_import::ImportDriver;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct CoreController {
    pub cli: CratesProCli,
}
struct SharedState {
    is_packaging: bool,
}

impl CoreController {
    pub async fn new(cli: CratesProCli) -> Self {
        Self { cli }
    }

    pub async fn run(&self) {
        let shared_state: Arc<tokio::sync::Mutex<SharedState>> =
            Arc::new(Mutex::new(SharedState {
                is_packaging: false,
            }));

        let dont_clone = self.cli.dont_clone;

        let state_clone1: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let import_task = tokio::spawn(async move {
            // conduct repo parsing and importing
            let mut import_driver = ImportDriver::new(dont_clone).await;

            let _ = repo_import::reset_mq().await;

            loop {
                let mut state = state_clone1.lock().await;
                while state.is_packaging {
                    drop(state); // 释放锁以便等待
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    state = state_clone1.lock().await; // 重新获取锁
                }
                drop(state);
                import_driver.import_from_mq_for_a_message().await;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        let state_clone2: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let analyze_task = tokio::spawn(async move {
            loop {
                let mut state = state_clone2.lock().await;
                while state.is_packaging {
                    drop(state);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    state = state_clone2.lock().await;
                }
                drop(state);
                println!("Analyzing crate...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        let state_clone3: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let package_task = tokio::spawn(async move {
            loop {
                {
                    let mut state = state_clone3.lock().await;
                    state.is_packaging = true;
                }

                println!("Packaging results...");

                {
                    let mut state = state_clone3.lock().await;
                    state.is_packaging = false;
                }

                // after one hour
                tokio::time::sleep(Duration::from_secs(30 * 60)).await;
            }
        });

        import_task.await.unwrap();
        analyze_task.await.unwrap();
        package_task.await.unwrap();
    }
}
