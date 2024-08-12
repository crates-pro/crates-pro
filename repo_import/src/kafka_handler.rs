use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance, StreamConsumer};
use rdkafka::error::KafkaResult;
use rdkafka::message::{BorrowedMessage, Headers};
use rdkafka::{ClientContext, Message, TopicPartitionList};

use ssh2::Session;
use std::io::prelude::*;
use std::net::TcpStream;

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

pub struct KafkaHandler {
    consumer: LoggingConsumer,
}

impl KafkaHandler {
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

        KafkaHandler { consumer }
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

    /// reset the mq
    pub async fn reset_mq(&self) -> Result<(), Box<dyn std::error::Error>> {
        if std::env::var("HOST_PASSWORD").is_err() {
            panic!("Warning: HOST_PASSWORD environment variable is not set.");
        }

        tracing::info!(" Start reset Offset of Kafka.");
        let username = &std::env::var("HOST_USER_NAME")?;
        let password = &std::env::var("HOST_PASSWORD")?;
        let hostname = std::env::var("HOST_NAME")?;
        let port = 22;

        let tcp = TcpStream::connect((hostname, port))?;
        let mut sess = Session::new()?;

        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        sess.userauth_password(username, password)?;

        if !sess.authenticated() {
            panic!("Authentication failed!");
        }

        let command = r#"
        docker exec pensive_villani /opt/kafka/bin/kafka-consumer-groups.sh \
        --bootstrap-server localhost:9092 \
        --group default_group \
        --reset-offsets \
        --to-offset 0 \
        --execute \
        --topic REPO_SYNC_STATUS.dev
    "#;

        let mut channel = sess.channel_session()?;
        channel.exec(command)?;

        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        tracing::info!("Command output: {}", s);

        channel.send_eof()?;
        channel.wait_close()?;
        tracing::info!(
            "Finish reset Kafka MQ, Exit status: {}",
            channel.exit_status()?
        );

        Ok(())
    }
}
