use std::sync::Arc;

use init::database_connection;
use program_storage::ProgramStorage;

pub mod init;
pub mod program_storage;

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

    pub fn program_storage(&self) -> ProgramStorage {
        self.services.program_storage.clone()
    }
}
#[derive(Clone)]
pub struct Service {
    program_storage: ProgramStorage,
}

impl Service {
    async fn new(db_url: &str) -> Self {
        let connection = Arc::new(database_connection(db_url).await.unwrap());
        Self {
            program_storage: ProgramStorage::new(connection.clone()).await,
        }
    }

    async fn shared(db_url: &str) -> Arc<Self> {
        Arc::new(Self::new(db_url).await)
    }
}
