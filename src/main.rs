mod cli;
mod core_controller;
use cli::CratesProCli;
use core_controller::CoreController;
use structopt::StructOpt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = CratesProCli::from_args();

    let core_controller = CoreController::new(cli).await;
    core_controller.run().await;
}
