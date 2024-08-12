use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientContext, Message, TopicPartitionList};

pub struct CustomContext;

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

pub struct KafkaConsumer {
    consumer: LoggingConsumer,
}

impl KafkaConsumer {
    pub fn new(brokers: &str, group_id: &str, topics: &[&str]) -> Self {
        let context = CustomContext;
        let consumer: LoggingConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .set_log_level(RDKafkaLogLevel::Debug)
            .create_with_context(context)
            .expect("Consumer creation failed");

        consumer
            .subscribe(topics)
            .expect("Can't subscribe to specified topics");

        KafkaConsumer { consumer }
    }

    pub async fn consume_once(&self) -> Option<BorrowedMessage> {
        match self.consumer.recv().await {
            Err(e) => {
                tracing::warn!("Kafka error: {}", e);
                None
            }
            Ok(m) => {
                if let Some(headers) = m.headers() {
                    for header in headers.iter() {
                        tracing::info!("  Header {:#?}: {:?}", header.key, header.value);
                    }
                }
                self.consumer.commit_message(&m, CommitMode::Async).unwrap();
                Some(m)
            }
        }
    }
}
