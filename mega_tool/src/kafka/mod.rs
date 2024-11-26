use std::env;

use rdkafka::producer::FutureProducer;

pub mod producer;

pub fn get_producer() -> FutureProducer {
    let brokers = env::var("KAFKA_BROKER").unwrap();
    producer::new_producer(&brokers)
}
