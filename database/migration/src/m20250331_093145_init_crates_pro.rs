use extension::postgres::Type;
use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{EnumIter, Iterable},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_type(
                Type::create()
                    .as_enum(CrateTypeEnum)
                    .values(CrateType::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(SyncStatusEnum)
                    .values(SyncStatus::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RepoSyncResult::Table)
                    .if_not_exists()
                    .col(pk_auto(RepoSyncResult::Id))
                    .col(string(RepoSyncResult::CrateName))
                    .col(text_null(RepoSyncResult::GithubUrl))
                    .col(text(RepoSyncResult::MegaUrl))
                    .col(enumeration(
                        RepoSyncResult::Status,
                        Alias::new("sync_status_enum"),
                        SyncStatus::iter(),
                    ))
                    .col(enumeration(
                        RepoSyncResult::CrateType,
                        Alias::new("crate_type_enum"),
                        SyncStatus::iter(),
                    ))
                    .col(text_null(RepoSyncResult::ErrMessage))
                    .col(text(RepoSyncResult::Version))
                    .col(date_time(RepoSyncResult::CreatedAt))
                    .col(date_time(RepoSyncResult::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-repo_sync_result_crate_name")
                    .unique()
                    .table(RepoSyncResult::Table)
                    .col(RepoSyncResult::CrateName)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Programs::Table)
                    .if_not_exists()
                    .col(pk_auto(Programs::Id))
                    .col(string(Programs::Name))
                    .col(text(Programs::Description))
                    .col(string(Programs::Namespace))
                    .col(string(Programs::MaxVersion))
                    .col(text(Programs::GithubUrl))
                    .col(text(Programs::MegaUrl))
                    .col(text(Programs::DocUrl))
                    .col(text(Programs::ProgramType))
                    .col(big_integer(Programs::Downloads))
                    .col(text(Programs::Cratesio))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProgramVersions::Table)
                    .if_not_exists()
                    .col(pk_auto(ProgramVersions::Id))
                    .col(string(ProgramVersions::Name))
                    .col(string(ProgramVersions::Version))
                    .col(text_null(ProgramVersions::Documentation))
                    .col(text(ProgramVersions::VersionType))
                    .col(date_time(ProgramVersions::CreatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProgramDependencies::Table)
                    .if_not_exists()
                    .col(string(ProgramDependencies::NameAndVersion))
                    .col(text(ProgramDependencies::DependencyName))
                    .col(text(ProgramDependencies::DependencyVersion))
                    .primary_key(
                        Index::create()
                            .col(ProgramDependencies::NameAndVersion)
                            .col(ProgramDependencies::DependencyName)
                            .col(ProgramDependencies::DependencyVersion),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RepoSyncResult {
    Table,
    Id,
    CrateName,
    GithubUrl,
    MegaUrl,
    CrateType,
    Status,
    ErrMessage,
    CreatedAt,
    UpdatedAt,
    Version,
}

#[derive(DeriveIden)]
enum Programs {
    Table,
    Id,
    Name,
    Description,
    Namespace,
    MaxVersion,
    GithubUrl,
    MegaUrl,
    DocUrl,
    ProgramType,
    Downloads,
    Cratesio,
}

#[derive(DeriveIden)]
enum ProgramVersions {
    Table,
    Id,
    Name,
    Version,
    Documentation,
    VersionType,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ProgramDependencies {
    Table,
    NameAndVersion,
    DependencyName,
    DependencyVersion,
}

#[derive(DeriveIden)]
struct CrateTypeEnum;
#[derive(Iden, EnumIter)]
pub enum CrateType {
    Lib,
    Application,
}

#[derive(DeriveIden)]
struct SyncStatusEnum;
#[derive(Iden, EnumIter)]
pub enum SyncStatus {
    Syncing,
    Succeed,
    Failed,
    Analysing,
    Analysed,
}
