use backend::run_quary_server;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    run_quary_server().unwrap();
}
