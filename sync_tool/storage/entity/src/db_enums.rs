use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum RepoSyncStatus {
    #[sea_orm(string_value = "syncing")]
    Syncing,
    #[sea_orm(string_value = "succeed")]
    Succeed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "analysing")]
    Analysing,
    #[sea_orm(string_value = "analysed")]
    Analysed,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum CrateType {
    #[sea_orm(string_value = "lib")]
    Lib,
    #[sea_orm(string_value = "application")]
    Application,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum MessageKind {
    #[sea_orm(string_value = "mega")]
    Mega,
    #[sea_orm(string_value = "user")]
    User,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum SourceOfData {
    #[sea_orm(string_value = "cratesio")]
    Cratesio,
    #[sea_orm(string_value = "github")]
    Github,
}
