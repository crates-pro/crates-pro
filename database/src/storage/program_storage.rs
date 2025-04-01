use std::sync::Arc;

use entity::programs::{self};
use sea_orm::{sea_query::OnConflict, DatabaseConnection, DbErr, EntityTrait};

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
        let on_conflict = OnConflict::column(programs::Column::Id)
            .do_nothing()
            .to_owned();

        programs::Entity::insert_many(models)
            .on_conflict(on_conflict)
            .do_nothing()
            .exec(self.get_connection())
            .await.unwrap();
        Ok(())
    }
}
