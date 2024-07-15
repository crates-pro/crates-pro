use crates_sync::{consumer::MessageCallback, repo_sync_model};
use rdkafka::{message::BorrowedMessage, Message};
use tokio::time::{sleep, Duration};

#[derive(Default, Debug)]
pub struct RepoSyncCallback {
    pub entry: Option<repo_sync_model::Model>,
}

impl MessageCallback for RepoSyncCallback {
    fn on_message(&mut self, m: &BorrowedMessage) {
        let model = match serde_json::from_slice::<repo_sync_model::Model>(m.payload().unwrap()) {
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

        self.entry = model;

        tokio::spawn(async move {
            sleep(Duration::from_millis(1000)).await;
        });
    }
}
