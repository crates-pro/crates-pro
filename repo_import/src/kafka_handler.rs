use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::producer::{BaseProducer, BaseRecord, ProducerContext};
use rdkafka::util::Timeout;
use rdkafka::{ClientContext, Message, TopicPartitionList};
use std::process::Command;

#[derive(Clone)]
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

impl ProducerContext for CustomContext {
    type DeliveryOpaque = ();

    fn delivery(
        &self,
        result: &rdkafka::producer::DeliveryResult,
        _delivery_opaque: Self::DeliveryOpaque,
    ) {
        match result {
            Ok(delivery) => tracing::info!("Delivered message to {:?}", delivery),
            Err((error, _)) => tracing::error!("Failed to deliver message: {:?}", error),
        }
    }
}

type LoggingConsumer = StreamConsumer<CustomContext>;

pub struct KafkaHandler {
    consumer: LoggingConsumer,
    producer: BaseProducer<CustomContext>,
}

impl KafkaHandler {
    pub fn new(brokers: &str, group_id: &str) -> Self {
        let context = CustomContext;

        let consumer: LoggingConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .set_log_level(RDKafkaLogLevel::Debug)
            .create_with_context(context.clone())
            .expect("Consumer creation failed");

        let producer: BaseProducer<CustomContext> = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create_with_context(context)
            .expect("Producer creation failed");

        KafkaHandler { consumer, producer }
    }

    pub async fn consume_once(&self, topic: &str) -> Option<BorrowedMessage> {
        self.consumer
            .subscribe(&[topic])
            .expect("Can't subscribe to specified topic");

        match self.consumer.recv().await {
            Err(e) => {
                tracing::warn!("Kafka error: {}", e);
                None
            }
            Ok(m) => {
                tracing::debug!("{:?}", m);
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

    pub fn send_message(&self, topic: &str, key: &str, payload: &str) {
        let record = BaseRecord::to(topic).key(key).payload(payload);

        match self.producer.send(record) {
            Ok(_) => tracing::info!("Message sent successfully"),
            Err(e) => tracing::error!("Failed to send message: {:?}", e),
        }

        self.producer.poll(Timeout::Never);
    }
}
/// reset the mq
pub async fn reset_kafka_offset() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("/opt/kafka/bin/kafka-consumer-groups.sh")
        .args([
            "--bootstrap-server",
            "210.28.134.203:30092",
            "--group",
            "default_group",
            "--reset-offsets",
            "--to-offset",
            "0",
            "--execute",
            "--topic",
            "REPO_SYNC_STATUS.dev.0902",
        ])
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        println!("Command executed successfully");
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Output: {}", stdout);
    } else {
        eprintln!("Command failed to execute");
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error: {}", stderr);
    }

    Ok(())
}
