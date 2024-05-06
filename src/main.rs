use router::run_quary_server;

fn main() {
    dotenvy::dotenv().ok();
    run_quary_server().unwrap();
}
