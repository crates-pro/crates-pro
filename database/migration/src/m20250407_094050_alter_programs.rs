use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Programs::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("github_analyzed"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("repo_created_at")).date_time(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(GithubUser::Table)
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("commit_email")).string())
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
enum Programs {
    Table,
}

#[derive(DeriveIden)]
enum GithubUser {
    Table,
}
