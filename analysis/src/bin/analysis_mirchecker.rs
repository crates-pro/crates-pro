use analysis::kafka_handler::KafkaReader;
use analysis::{ analyse_once_mirchecker};
use std::{
    env,
    fs::File,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    println!("Starting the program. Press Ctrl+C to stop.");
    dotenvy::dotenv().ok();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let log_path = format!("/var/target/log_mirchecker_{}.ans", timestamp);
    let file = File::create(&log_path).expect("Unable to create log file");
    tracing_subscriber::fmt()
        .with_writer(file)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting with log file: {}", log_path);
    let kafka_broker = env::var("KAFKA_BROKER").unwrap();
    let consumer_group_id = env::var("KAFKA_CONSUMER_GROUP_ID").unwrap();
    let analysis_topic = env::var("KAFKA_ANALYSIS_TOPIC").unwrap();
    tracing::info!(
        "{},{},{}",
        kafka_broker.clone(),
        consumer_group_id.clone(),
        analysis_topic.clone()
    );
    let kafka_reader = KafkaReader::new(&kafka_broker, &consumer_group_id, &analysis_topic);
    loop {
        tracing::info!("analysis_mirchecker");
        let output_dir_path = "/var/target/mirchecker-res/";

        #[allow(clippy::let_unit_value)]
        let _ = analyse_once_mirchecker(&kafka_reader, output_dir_path).await;
        thread::sleep(Duration::from_secs(0));
    }
}
