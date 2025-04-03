use std::sync::Arc;

use init::database_connection;
use github_handler_storage::GithubHanlderStorage;

pub mod init;
pub mod github_handler_storage;

#[derive(Clone)]
pub struct Context {
    pub services: Arc<Service>,
}

impl Context {
    pub async fn new(db_url: &str) -> Self {
        Context {
            services: Service::shared(db_url).await,
        }
    }

    pub fn github_handler_stg(&self) -> GithubHanlderStorage {
        self.services.github_handler_storage.clone()
    }
}
#[derive(Clone)]
pub struct Service {
    github_handler_storage: GithubHanlderStorage,
}

impl Service {
    async fn new(db_url: &str) -> Self {
        let connection = Arc::new(database_connection(db_url).await.unwrap());
        Self {
            github_handler_storage: GithubHanlderStorage::new(connection.clone()).await,
        }
    }

    async fn shared(db_url: &str) -> Arc<Self> {
        Arc::new(Self::new(db_url).await)
    }
}
