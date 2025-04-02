use std::sync::Arc;

use entity::{
    github_sync_status,
    programs::{self},
};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter,
};

#[derive(Clone)]
pub struct ProgramStorage {
    pub connection: Arc<DatabaseConnection>,
}

impl ProgramStorage {
    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    pub async fn new(connection: Arc<DatabaseConnection>) -> Self {
        ProgramStorage { connection }
    }

    pub async fn save_programs(&self, models: Vec<programs::ActiveModel>) -> Result<(), DbErr> {
        let on_conflict = OnConflict::column(programs::Column::GithubUrl)
            .do_nothing()
            .to_owned();

        programs::Entity::insert_many(models)
            .on_conflict(on_conflict)
            .do_nothing()
            .exec(self.get_connection())
            .await
            .unwrap();
        Ok(())
    }

    pub async fn save_github_sync_status(
        &self,
        model: github_sync_status::ActiveModel,
    ) -> Result<github_sync_status::ActiveModel, DbErr> {
        model.save(self.get_connection()).await
    }

    pub async fn get_github_sync_status_by_date(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Option<github_sync_status::Model>, DbErr> {
        let result = github_sync_status::Entity::find()
            .filter(github_sync_status::Column::StartDate.eq(start_date))
            .filter(github_sync_status::Column::EndDate.eq(end_date))
            .one(self.get_connection())
            .await?;
        Ok(result)
    }
}
