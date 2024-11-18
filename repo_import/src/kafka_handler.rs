use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::{BaseConsumer, CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::{KafkaError, KafkaResult};
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::producer::{BaseProducer, BaseRecord, ProducerContext};
use rdkafka::util::Timeout;
use rdkafka::{ClientContext, Message, TopicPartitionList};
use std::process::Command;
use std::time::Duration;

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
        _result: &rdkafka::producer::DeliveryResult,
        _delivery_opaque: Self::DeliveryOpaque,
    ) {
        // match result {
        //     Ok(delivery) => tracing::info!("Delivered message to {:?}", delivery),
        //     Err((error, _)) => tracing::error!("Failed to deliver message: {:?}", error),
        // }
    }
}

pub enum KafkaHandler {
    Consumer(BaseConsumer<CustomContext>),
    Producer(BaseProducer<CustomContext>),
}
impl KafkaHandler {
    pub fn new_consumer(brokers: &str, group_id: &str, topic: &str) -> Result<Self, KafkaError> {
        let context = CustomContext;

        let consumer: BaseConsumer<CustomContext> = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "10000")
            .set("heartbeat.interval.ms", "1500")
            .set("max.poll.interval.ms", "3000000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .set_log_level(RDKafkaLogLevel::Debug)
            .create_with_context(context)?;

        consumer.subscribe(&[topic])?;

        Ok(KafkaHandler::Consumer(consumer))
    }

    pub fn new_producer(brokers: &str) -> Result<Self, KafkaError> {
        let context = CustomContext;

        let producer: BaseProducer<CustomContext> = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create_with_context(context)?;

        Ok(KafkaHandler::Producer(producer))
    }

    pub async fn consume_once(&self) -> Result<BorrowedMessage, KafkaError> {
        if let KafkaHandler::Consumer(consumer) = self {
            tracing::debug!("Trying to consume a message");

            match consumer.poll(Duration::from_secs(1)) {
                None => {
                    tracing::info!("No message received");
                    Err(KafkaError::NoMessageReceived)
                }
                Some(m) => {
                    let m = m?;
                    tracing::debug!("{:?}", m);
                    if let Some(headers) = m.headers() {
                        for header in headers.iter() {
                            tracing::info!("Header {}: {:?}", header.key, header.value);
                        }
                    }
                    consumer.commit_message(&m, CommitMode::Async).unwrap();
                    Ok(m)
                }
            }
        } else {
            unreachable!("Called consume_once on a producer");
        }
    }

    pub async fn send_message(&self, topic: &str, key: &str, payload: &str) {
        if let KafkaHandler::Producer(producer) = self {
            let record = BaseRecord::to(topic).key(key).payload(payload);

            match producer.send(record) {
                Ok(_) => {
                    tracing::info!("Message sent successfully");
                }
                Err(e) => tracing::error!("Failed to send message: {:?}", e),
            }

            producer.poll(Timeout::Never);
        } else {
            tracing::error!("Called send_message on a consumer");
        }
    }
}

/// reset the mq
pub async fn reset_kafka_offset() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Start to reset import kafka");
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

    tracing::info!("Finish to reset import kafka");
    Ok(())
}
