use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub crate_name: String,
    pub github_url: Option<String>,
    pub mega_url: String,
    pub crate_type: CrateType,
    pub status: RepoSyncStatus,
    pub err_message: Option<String>,
    pub created_at: NaiveDate,
    pub updated_at: NaiveDate,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum CrateType {
    Lib,
    Application,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum RepoSyncStatus {
    Syncing,
    Succeed,
    Failed,
    Analysing,
    Analysed,
}
