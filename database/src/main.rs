mod crate_info;
mod load_plugin;
mod quary_server;
mod tu_client;

use load_plugin::load_plugin;
use quary_server::start_quary_server;
use tokio;

#[tokio::main]
async fn main() {
    load_plugin("").await;
    start_quary_server().await;
}
