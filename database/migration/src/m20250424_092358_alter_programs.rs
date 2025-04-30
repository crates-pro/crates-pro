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
                        ColumnDef::new(Alias::new("github_node_id"))
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("updated_at")).date_time())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(GithubUser::Table)
                    .drop_column(Alias::new("commit_email"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Crates::Table)
                    .add_column_if_not_exists(ColumnDef::new(Alias::new("github_node_id")).string())
                    .add_column_if_not_exists(
                        ColumnDef::new(Alias::new("repo_invalid"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-programs_github_node_id_unique")
                    .unique()
                    .table(Programs::Table)
                    .col(Programs::GithubNodeId)
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
    GithubNodeId,
}

#[derive(DeriveIden)]
enum Crates {
    Table,
}

#[derive(DeriveIden)]
enum GithubUser {
    Table,
}