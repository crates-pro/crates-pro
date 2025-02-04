mod cli;
mod core_controller;

use cli::CratesProCli;
use core_controller::CoreController;
use std::fs::File;
use std::time::{SystemTime, UNIX_EPOCH};
use structopt::StructOpt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // 获取当前时间戳
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 创建带时间戳的日志文件
    let log_path = format!("target/log_{}.ans", timestamp);
    let file = File::create(&log_path).expect("Unable to create log file");

    // 设置日志记录器
    tracing_subscriber::fmt()
        .with_writer(file)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting with log file: {}", log_path);

    let cli = CratesProCli::from_args();
    let core_controller = CoreController::new(cli).await;
    core_controller.run().await;
}
