use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE repository_contributor
                ALTER COLUMN repository_id TYPE UUID
                USING repository_id::uuid;
                "#,
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE contributor_location
                ALTER COLUMN repository_id TYPE UUID
                USING repository_id::uuid;
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}