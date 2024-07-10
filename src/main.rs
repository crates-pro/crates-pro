use backend::run_quary_server;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    run_quary_server().unwrap();
}
