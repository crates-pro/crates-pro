use std::sync::Arc;

use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientContext, Message, TopicPartitionList};
use tokio::sync::Mutex;

pub struct CustomContext;

pub trait MessageCallback {
    fn on_message(&mut self, message: &BorrowedMessage);
}

impl ClientContext for CustomContext {}

impl ConsumerContext for CustomContext {
    fn pre_rebalance(&self, rebalance: &Rebalance) {
        tracing::info!("Pre rebalance {:?}", rebalance);
    }

    fn post_rebalance(&self, rebalance: &Rebalance) {
        tracing::info!("Post rebalance {:?}", rebalance);
    }

    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        tracing::info!("Committing offsets: {:?}", result);
    }
}

// A type alias with your custom consumer can be created for convenience.
type LoggingConsumer = StreamConsumer<CustomContext>;

pub async fn consume(
    brokers: &str,
    group_id: &str,
    topics: &[&str],
    callback: Arc<Mutex<dyn MessageCallback>>,
) {
    let context = CustomContext;
    let consumer: LoggingConsumer = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        //.set("statistics.interval.ms", "30000")
        //.set("auto.offset.reset", "smallest")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create_with_context(context)
        .expect("Consumer creation failed");

    consumer
        .subscribe(topics)
        .expect("Can't subscribe to specified topics");
    loop {
        match consumer.recv().await {
            Err(e) => tracing::warn!("Kafka error: {}", e),
            Ok(m) => {
                callback.lock().await.on_message(&m);
                if let Some(headers) = m.headers() {
                    for header in headers.iter() {
                        tracing::info!("  Header {:#?}: {:?}", header.key, header.value);
                    }
                }
                consumer.commit_message(&m, CommitMode::Async).unwrap();
                // consumer.store_offset_from_message(&m).unwrap();
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;
    use std::{env, sync::Arc};

    use rdkafka::{message::BorrowedMessage, Message};
    use tokio::sync::Mutex;

    use crate::consumer::consume;
    use crate::consumer::MessageCallback;
    use crate::repo_sync_model;

    struct MockCallback;

    impl MessageCallback for MockCallback {
        fn on_message(&mut self, m: &BorrowedMessage) {
            let model = match serde_json::from_slice::<repo_sync_model::Model>(m.payload().unwrap())
            {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("Error while deserializing message payload: {:?}", e);
                    None
                }
            };
            tracing::info!(
            "key: '{:?}', payload: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
            m.key(),
            model,
            m.topic(),
            m.partition(),
            m.offset(),
            m.timestamp()
        );
            thread::sleep(Duration::from_millis(1000));
        }
    }

    #[tokio::test]
    async fn test_consume() {
        dotenvy::dotenv().ok();
        tracing_subscriber::fmt::init();
        let broker = env::var("KAFKA_BROKER").unwrap();
        let topic = env::var("KAFKA_TOPIC").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();
        let mut callback = Arc::new(Mutex::new(MockCallback));
        tracing::info!("{},{},{}", broker, topic, group_id);
        consume(&broker, &group_id, &[&topic], callback).await;
    }
}
