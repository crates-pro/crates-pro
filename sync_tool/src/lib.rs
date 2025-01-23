use entity::{db_enums::CrateType, repo_sync_status};
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};

pub mod command;
pub mod crate_to_repo;
pub mod handle_repo;
pub mod incremental_update;
pub mod kafka;
pub mod sync_crate_to_repo;
pub mod util;

pub async fn get_record(
    conn: &DatabaseConnection,
    crate_name: &str,
) -> repo_sync_status::ActiveModel {
    let model = repo_sync_status::Entity::find()
        .filter(repo_sync_status::Column::CrateName.eq(crate_name))
        .one(conn)
        .await
        .unwrap();

    if model.is_none() {
        repo_sync_status::ActiveModel {
            id: NotSet,
            crate_name: Set(crate_name.to_owned()),
            github_url: Set(None),
            mega_url: NotSet,
            crate_type: Set(CrateType::Lib),
            status: NotSet,
            err_message: Set(None),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            version: Set("0.0.0".to_string()),
        }
    } else {
        let res = model.unwrap();
        res.into_active_model()
    }
}
