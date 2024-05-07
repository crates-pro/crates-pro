use serde::{Deserialize, Serialize};

use entity::{
    db_enums::{CrateType, RepoSyncStatus},
    repo_sync_status,
};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct RepoSync {
    pub id: i32,
    pub crate_name: String,
    pub mega_url: String,
    pub crate_type: CrateType,
    pub status: RepoSyncStatus,
}

impl From<repo_sync_status::Model> for RepoSync {
    fn from(value: repo_sync_status::Model) -> Self {
        Self {
            id: value.id,
            crate_name: value.crate_name,
            mega_url: value.mega_url,
            crate_type: value.crate_type,
            status: value.status,
        }
    }
}
