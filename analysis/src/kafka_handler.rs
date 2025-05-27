use model::general_model::VersionWithTag;
use model::repo_sync_model;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::{ClientConfig, Message};

pub struct KafkaReader {
    consumer: StreamConsumer,
}

impl KafkaReader {
    pub fn new(broker: &str, group_id: &str, topic: &str) -> Self {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", broker)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "10000")
            .set("heartbeat.interval.ms", "1500")
            .set("max.poll.interval.ms", "3000000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Consumer creation failed");
        consumer
            .subscribe(&[topic])
            .expect("Can't subscribe to specified topic");
        KafkaReader { consumer }
    }
    #[allow(clippy::needless_return)]
    pub async fn read_single_message(&self) -> Result<repo_sync_model::MessageModel, KafkaError> {
        tracing::info!("enter read_single_message");
        tracing::info!("enter read_single_message loop");

        match self.consumer.recv().await {
            Ok(message) => {
                tracing::info!("enter get message");
                if let Some(payload) = message.payload() {
                    tracing::info!("enter message if");
                    match serde_json::from_slice::<repo_sync_model::MessageModel>(payload) {
                        Ok(version_with_tag) => {
                            tracing::info!("enter message match");
                            return Ok(version_with_tag);
                        }
                        Err(e) => {
                            tracing::info!("Failed to deserialize message: {:?}", e);
                            return Err(KafkaError::NoMessageReceived);
                        }
                    }
                } else {
                    return Err(KafkaError::NoMessageReceived);
                }
            }
            Err(e) => {
                tracing::info!("Error receiving message: {}", e);
                return Err(KafkaError::NoMessageReceived);
            }
        }
    }
    #[allow(clippy::needless_return)]
    pub async fn read_single_message_mirchecker(&self) -> Result<VersionWithTag, KafkaError> {
        tracing::info!("enter read_single_message");
        tracing::info!("enter read_single_message loop");

        match self.consumer.recv().await {
            Ok(message) => {
                tracing::info!("enter get message");
                if let Some(payload) = message.payload() {
                    tracing::info!("enter message if");
                    match serde_json::from_slice::<VersionWithTag>(payload) {
                        Ok(version_with_tag) => {
                            tracing::info!("enter message match");
                            return Ok(version_with_tag);
                        }
                        Err(e) => {
                            tracing::info!("Failed to deserialize message: {:?}", e);
                            return Err(KafkaError::NoMessageReceived);
                        }
                    }
                } else {
                    return Err(KafkaError::NoMessageReceived);
                }
            }
            Err(e) => {
                tracing::info!("Error receiving message: {}", e);
                return Err(KafkaError::NoMessageReceived);
            }
        }
    }
}
