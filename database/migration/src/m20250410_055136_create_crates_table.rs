use sea_orm_migration::{
    prelude::*,
    schema::{pk_uuid, string_null, text_null},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Crates::Table)
                    .if_not_exists()
                    .col(pk_uuid(Crates::Id)) // 使用 UUID 作为主键
                    .col(string_null(Crates::Name))
                    .col(text_null(Crates::Repository))
                    .to_owned(),
            )
            .await?;

        // 为 name 字段创建唯一索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx-crates-name")
                    .table(Crates::Table)
                    .col(Crates::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Crates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Crates {
    Table,
    Id,
    Name,
    Repository,
}
