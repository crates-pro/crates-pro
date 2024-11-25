mod cli;
mod core_controller;

use cli::CratesProCli;
use core_controller::CoreController;
use std::fs::File;
use structopt::StructOpt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // 创建一个文件
    let file = File::create("target/log.ans").expect("Unable to create log file");

    // 设置日志记录器
    tracing_subscriber::fmt()
        .with_writer(file) // 直接使用 File
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = CratesProCli::from_args();

    let core_controller = CoreController::new(cli).await;
    core_controller.run().await;
}
