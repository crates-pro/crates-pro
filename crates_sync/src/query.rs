use std::sync::Arc;

use entity::{db_enums::RepoSyncStatus, repo_sync_status};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::repo_sync_model::RepoSync;

#[derive(Clone)]
pub struct MegaStorage {
    pub connection: Arc<DatabaseConnection>,
}

impl MegaStorage {
    pub fn new(connection: Arc<DatabaseConnection>) -> Self {
        MegaStorage { connection }
    }

    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    pub async fn get_all_repos(&self) -> Vec<RepoSync> {
        repo_sync_status::Entity::find()
            .filter(repo_sync_status::Column::Status.eq(RepoSyncStatus::Succeed))
            .all(self.get_connection())
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.into())
            .collect()
    }
}
