use sea_orm::{DeriveActiveEnum, EnumIter};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
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

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(Some(1))")]
pub enum CrateType {
    #[sea_orm(string_value = "lib")]
    Lib,
    #[sea_orm(string_value = "application")]
    Application,
}

