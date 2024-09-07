use model::general_model::VersionWithTag;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::{ClientConfig, Message};
use std::time::Duration;

pub struct KafkaReader {
    consumer: BaseConsumer,
}

impl KafkaReader {
    pub fn new(broker: &str, group_id: &str) -> Self {
        let consumer: BaseConsumer = ClientConfig::new()
            .set("bootstrap.servers", broker)
            .set("group.id", group_id)
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Consumer creation failed");
        KafkaReader { consumer }
    }

    pub fn read_single_message(&self, topic: &str) -> Option<VersionWithTag> {
        self.consumer
            .subscribe(&[topic])
            .expect("Subscription failed");

        loop {
            match self.consumer.poll(Duration::from_secs(1)) {
                Some(Ok(message)) => {
                    if let Some(payload) = message.payload() {
                        match serde_json::from_slice::<VersionWithTag>(payload) {
                            Ok(version_with_tag) => return Some(version_with_tag),
                            Err(e) => eprintln!("Failed to deserialize message: {:?}", e),
                        }
                    }
                }
                Some(Err(e)) => eprintln!("Kafka error: {:?}", e),
                None => return None,
            }
        }
    }
}
