use analysis::analyse_once_mirchecker;
use analysis::kafka_handler::KafkaReader;
use analysis::utils::{init_logger, load_env};
use std::{thread, time::Duration};

#[tokio::main]
async fn main() {
    println!("Starting the program. Press Ctrl+C to stop.");
    let _log_file = init_logger("mirchecker");
    let output_dir_path = "/var/target/mirchecker-res/";

    let (kafka_broker, consumer_group_id, analysis_topic) = load_env();
    let kafka_reader = KafkaReader::new(&kafka_broker, &consumer_group_id, &analysis_topic);

    loop {
        #[allow(clippy::let_unit_value)]
        let _ = analyse_once_mirchecker(&kafka_reader, output_dir_path).await;
        thread::sleep(Duration::from_secs(0));
    }
}
