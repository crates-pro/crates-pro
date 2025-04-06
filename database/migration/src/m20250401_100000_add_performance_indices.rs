use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 添加性能优化索引
///
/// 此迁移添加针对常用查询的索引，特别是优化顶级贡献者查询
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 添加组合索引：repository_id + contributions DESC
        // 这将大大加速按repository_id筛选并按contributions排序的查询
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repo_contrib_repo_id_contributions")
                    .table(RepositoryContributor::Table)
                    .col(RepositoryContributor::RepositoryId)
                    .col(RepositoryContributor::Contributions)
                    .to_owned(),
            )
            .await?;

        // 为repository_contributor表的contributions列添加索引（用于ORDER BY）
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repository_contributor_contributions")
                    .table(RepositoryContributor::Table)
                    .col(RepositoryContributor::Contributions)
                    .to_owned(),
            )
            .await?;

        // 为repository_contributor表的repository_id列添加索引（用于WHERE条件）
        // 虽然已经有组合唯一索引，但单独的索引可能更高效用于仅按repository_id筛选
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repository_contributor_repo_id")
                    .table(RepositoryContributor::Table)
                    .col(RepositoryContributor::RepositoryId)
                    .to_owned(),
            )
            .await?;

        // 为github_user表的id列添加索引（如果不是主键）
        // 通常主键已经自动索引，所以这可能是多余的
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_github_user_id")
                    .table(GithubUser::Table)
                    .col(GithubUser::Id)
                    .to_owned(),
            )
            .await?;

        // 为二次查询添加索引：中国贡献者查询
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_location_is_from_china")
                    .table(ContributorLocation::Table)
                    .col(ContributorLocation::IsFromChina)
                    .to_owned(),
            )
            .await?;

        // 添加额外的优化索引

        // 为中国贡献者查询优化 - 复合索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_location_repo_id_is_china")
                    .table(ContributorLocation::Table)
                    .col(ContributorLocation::RepositoryId)
                    .col(ContributorLocation::IsFromChina)
                    .to_owned(),
            )
            .await?;

        // GitHub用户登录名索引 - 用于按登录名查询用户
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_github_user_login")
                    .table(GithubUser::Table)
                    .col(GithubUser::Login)
                    .to_owned(),
            )
            .await?;

        // 程序GitHub URL索引 - 用于按GitHub URL查询项目
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_programs_github_url")
                    .table(Programs::Table)
                    .col(Programs::GithubUrl)
                    .to_owned(),
            )
            .await?;

        // 贡献者位置查询的复合索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_location_repo_user")
                    .table(ContributorLocation::Table)
                    .col(ContributorLocation::RepositoryId)
                    .col(ContributorLocation::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除添加的额外索引
        manager
            .drop_index(
                Index::drop()
                    .name("idx_contributor_location_repo_id_is_china")
                    .table(ContributorLocation::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_github_user_login")
                    .table(GithubUser::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_programs_github_url")
                    .table(Programs::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_contributor_location_repo_user")
                    .table(ContributorLocation::Table)
                    .to_owned(),
            )
            .await?;

        // 删除原有的索引
        manager
            .drop_index(
                Index::drop()
                    .name("idx_repo_contrib_repo_id_contributions")
                    .table(RepositoryContributor::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_repository_contributor_contributions")
                    .table(RepositoryContributor::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_repository_contributor_repo_id")
                    .table(RepositoryContributor::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_github_user_id")
                    .table(GithubUser::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_contributor_location_is_from_china")
                    .table(ContributorLocation::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

/// Repository Contributors
#[derive(DeriveIden)]
enum RepositoryContributor {
    Table,
    RepositoryId,
    Contributions,
}

/// Github User
#[derive(DeriveIden)]
enum GithubUser {
    Table,
    Id,
    Login,
}

/// Contributor Locations
#[derive(DeriveIden)]
enum ContributorLocation {
    Table,
    RepositoryId,
    UserId,
    IsFromChina,
}

/// Programs
#[derive(DeriveIden)]
enum Programs {
    Table,
    GithubUrl,
}
