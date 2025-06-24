use analysis::kafka_handler::KafkaReader;
use analysis::utils::load_env;
use analysis::{analyse_once, utils::init_logger};
use std::{thread, time::Duration};

#[tokio::main]
async fn main() {
    println!("Starting the program. Press Ctrl+C to stop.");
    let _log_file = init_logger("senseleak");
    let output_dir_path = "/var/target/senseleak-res/";

    let (kafka_broker, consumer_group_id, analysis_topic) = load_env();
    let kafka_reader = KafkaReader::new(&kafka_broker, &consumer_group_id, &analysis_topic);

    loop {
        #[allow(clippy::let_unit_value)]
        let _ = analyse_once(&kafka_reader, output_dir_path).await;
        thread::sleep(Duration::from_secs(0));
    }
}
