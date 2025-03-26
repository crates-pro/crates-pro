use analysis::analyse_once;
//use analysis::kafka_handler;
use analysis::kafka_handler::KafkaReader;
use std::fs::File;
//use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, thread};
use tracing_subscriber::EnvFilter;
#[tokio::main]
async fn main() {
    println!("Starting the program. Press Ctrl+C to stop.");
    dotenvy::dotenv().ok();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let log_path = format!("/var/target/log_{}.ans", timestamp);
    let file = File::create(&log_path).expect("Unable to create log file");
    tracing_subscriber::fmt()
        .with_writer(file)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting with log file: {}", log_path);
    //let should_reset_kafka_offset = env::var("SHOULD_RESET_KAFKA_OFFSET").unwrap().eq("1");
    /*if should_reset_kafka_offset {
        reset_kafka_offset().await.unwrap();
    }*/
    let kafka_broker = env::var("KAFKA_BROKER").unwrap();
    let consumer_group_id = env::var("KAFKA_CONSUMER_GROUP_ID").unwrap();
    let analysis_topic = env::var("KAFKA_IMPORT_TOPIC").unwrap();
    tracing::info!(
        "{},{},{}",
        kafka_broker.clone(),
        consumer_group_id.clone(),
        analysis_topic.clone()
    );
    let kafka_reader = KafkaReader::new(&kafka_broker, &consumer_group_id, &analysis_topic);
    loop {
        tracing::info!("analysis_tool_worker");
        let output_dir_path = "/var/target/senseleak-res/";

        /*match fs::create_dir(output_dir_path) {
            Ok(_) => {}
            Err(_) => {}
        }*/
        #[allow(clippy::let_unit_value)]
        let _ = analyse_once(&kafka_reader, output_dir_path).await;
        thread::sleep(Duration::from_secs(0));
    }
}
