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
                    .col(pk_uuid(Programs::Id))
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
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-programs_github_url")
                    .table(Programs::Table)
                    .col(Programs::GithubUrl)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GithubSyncStatus::Table)
                    .if_not_exists()
                    .col(pk_auto(GithubSyncStatus::Id))
                    .col(string(GithubSyncStatus::StartDate))
                    .col(string(GithubSyncStatus::EndDate))
                    .col(boolean(GithubSyncStatus::SyncResult))
                    .to_owned(),
            )
            .await?;

        // 创建github_users表
        manager
            .create_table(
                Table::create()
                    .table(GithubUser::Table)
                    .if_not_exists()
                    .col(pk_auto(GithubUser::Id))
                    .col(big_integer(GithubUser::GithubId).unique_key())
                    .col(string(GithubUser::Login))
                    .col(string_null(GithubUser::Name))
                    .col(string_null(GithubUser::Email))
                    .col(text_null(GithubUser::AvatarUrl))
                    .col(string_null(GithubUser::Company))
                    .col(string_null(GithubUser::Location))
                    .col(text_null(GithubUser::Bio))
                    .col(integer_null(GithubUser::PublicRepos))
                    .col(integer_null(GithubUser::Followers))
                    .col(integer_null(GithubUser::Following))
                    .col(date_time(GithubUser::CreatedAt))
                    .col(date_time(GithubUser::UpdatedAt))
                    .col(date_time(GithubUser::InsertedAt).default(Expr::current_timestamp()))
                    .col(date_time(GithubUser::UpdatedAtLocal).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        // 创建repository_contributors表
        manager
            .create_table(
                Table::create()
                    .table(RepositoryContributor::Table)
                    .if_not_exists()
                    .col(pk_auto(RepositoryContributor::Id))
                    .col(string(RepositoryContributor::RepositoryId))
                    .col(integer(RepositoryContributor::UserId).integer().not_null())
                    .col(integer(RepositoryContributor::Contributions).default(0))
                    .col(
                        date_time(RepositoryContributor::InsertedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(RepositoryContributor::UpdatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建contributor_locations表
        manager
            .create_table(
                Table::create()
                    .table(ContributorLocation::Table)
                    .if_not_exists()
                    .col(pk_auto(ContributorLocation::Id))
                    .col(string(ContributorLocation::RepositoryId))
                    .col(integer(ContributorLocation::UserId))
                    .col(boolean(ContributorLocation::IsFromChina))
                    .col(string_null(ContributorLocation::CommonTimezone))
                    .col(
                        date_time(ContributorLocation::AnalyzedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 添加唯一约束
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repository_contributors_unique")
                    .table(RepositoryContributor::Table)
                    .col(RepositoryContributor::RepositoryId)
                    .col(RepositoryContributor::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_locations_unique")
                    .table(ContributorLocation::Table)
                    .col(ContributorLocation::RepositoryId)
                    .col(ContributorLocation::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(ProgramVersions::Table)
                    .if_not_exists()
                    .col(pk_uuid(ProgramVersions::Id))
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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-repo_sync_result_crate_name")
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(RepoSyncResult::Table).to_owned())
            .await?;
        // 删除type 防止启动的时候提示重复
        manager
            .drop_type(Type::drop().if_exists().name(CrateTypeEnum).to_owned())
            .await?;
        manager
            .drop_type(Type::drop().if_exists().name(SyncStatusEnum).to_owned())
            .await?;
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
enum GithubSyncStatus {
    Table,
    Id,
    StartDate,
    EndDate,
    SyncResult,
}

/// Github Users
#[derive(DeriveIden)]
enum GithubUser {
    Table,
    Id,
    GithubId,
    Login,
    Name,
    Email,
    AvatarUrl,
    Company,
    Location,
    Bio,
    PublicRepos,
    Followers,
    Following,
    CreatedAt,
    UpdatedAt,
    InsertedAt,
    UpdatedAtLocal,
}

/// Repository Contributors
#[derive(DeriveIden)]
enum RepositoryContributor {
    Table,
    Id,
    RepositoryId,
    UserId,
    Contributions,
    InsertedAt,
    UpdatedAt,
}

/// Contributor Locations
#[derive(DeriveIden)]
enum ContributorLocation {
    Table,
    Id,
    RepositoryId,
    UserId,
    IsFromChina,
    CommonTimezone,
    AnalyzedAt,
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
enum CrateType {
    Lib,
    Application,
}

#[derive(DeriveIden)]
struct SyncStatusEnum;
#[derive(Iden, EnumIter)]
enum SyncStatus {
    Syncing,
    Succeed,
    Failed,
    Analysing,
    Analysed,
}
