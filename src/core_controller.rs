//! the core controller module
//! It receives messages from Kafka MQ one by one,
//! parse them, and store it into tugraph, and notify
//! other processes.

use analysis::analyse_once;
use data_transporter::Transporter;
use repo_import::ImportDriver;

use crate::cli::CratesProCli;
use std::{env, fs, sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct CoreController {
    pub cli: CratesProCli,

    pub import: bool,
    pub analysis: bool,
    pub package: bool,
}
struct SharedState {
    is_packaging: bool,
}

impl CoreController {
    pub async fn new(cli: CratesProCli) -> Self {
        let import = env::var("CRATES_PRO_IMPORT").unwrap().eq("1");
        let analysis = env::var("CRATES_PRO_ANALYSIS").unwrap().eq("1");
        let package = env::var("CRATES_PRO_PACKAGE").unwrap().eq("1");
        Self {
            cli,
            import,
            analysis,
            package,
        }
    }

    pub async fn run(&self) {
        let import = self.import;
        let analysis = self.analysis;
        let package = self.package;

        let shared_state: Arc<tokio::sync::Mutex<SharedState>> =
            Arc::new(Mutex::new(SharedState {
                is_packaging: false,
            }));

        let dont_clone = self.cli.dont_clone;

        let state_clone1: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let import_task = tokio::spawn(async move {
            if import {
                repo_import::reset_kafka_offset()
                    .await
                    .unwrap_or_else(|x| panic!("{}", x));

                // conduct repo parsing and importing
                let mut import_driver = ImportDriver::new(dont_clone).await;

                loop {
                    let mut state = state_clone1.lock().await;
                    while state.is_packaging {
                        drop(state); // 释放锁以便等待
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        state = state_clone1.lock().await; // 重新获取锁
                    }

                    let _ = import_driver.import_from_mq_for_a_message().await;
                    drop(state);

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });

        let state_clone2: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let analyze_task = tokio::spawn(async move {
            if analysis {
                loop {
                    let mut state = state_clone2.lock().await;
                    while state.is_packaging {
                        drop(state);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        state = state_clone2.lock().await;
                    }
                    drop(state);
                    println!("Analyzing crate...");

                    let output_dir_path = "target/analysis";
                    fs::create_dir(output_dir_path).unwrap();
                    let _ = analyse_once(output_dir_path).await;

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });

        let state_clone3: Arc<tokio::sync::Mutex<SharedState>> = Arc::clone(&shared_state);
        let package_task = tokio::spawn(async move {
            if package {
                loop {
                    {
                        let mut state = state_clone3.lock().await;
                        state.is_packaging = true;
                    }

                    // process here
                    {
                        let mut transporter = Transporter::new(
                            "bolt://172.17.0.1:7687",
                            "admin",
                            "73@TuGraph",
                            "cratespro",
                        )
                        .await;

                        transporter.transport_data().await.unwrap();
                    }

                    {
                        let mut state = state_clone3.lock().await;
                        state.is_packaging = false;
                    }

                    // after one hour
                    tokio::time::sleep(Duration::from_secs(30 * 60)).await;
                }
            }
        });

        import_task.await.unwrap();
        analyze_task.await.unwrap();
        package_task.await.unwrap();
    }
}
