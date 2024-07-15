mod cli;
mod core_controller;
use cli::CratesProCli;
use core_controller::CoreController;
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = &CratesProCli::from_args();

    let core_controller = CoreController { cli };
    core_controller.run().await;
}
