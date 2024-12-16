use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord};

pub fn new_producer(brokers: &str) -> FutureProducer {
    ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error")
}

pub async fn send_message(producer: &FutureProducer, topic_name: &str, kafka_payload: String) {
    let producer = producer.clone();
    let topic_name = topic_name.to_owned();
    let kafka_payload = kafka_payload.to_owned();
    tokio::spawn(async move {
        let produce_future = producer.send(
            FutureRecord::to(&topic_name)
                .key("some key")
                .payload(&kafka_payload)
                .headers(OwnedHeaders::new().insert(Header {
                    key: "header_key",
                    value: Some("header_value"),
                })),
            Duration::from_secs(0),
        );
        match produce_future.await {
            Ok(delivery) => tracing::info!("Sent: {:?}", delivery),
            Err((e, _)) => tracing::error!("Error: {:?}", e),
        }
    });
}
