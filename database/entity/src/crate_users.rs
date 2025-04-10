//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.8

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "crate_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub gh_access_token: String,
    pub gh_login: String,
    pub name: Option<String>,
    pub gh_avatar: Option<String>,
    #[sea_orm(unique)]
    pub gh_id: i32,
    pub account_lock_reason: Option<String>,
    pub account_lock_until: Option<DateTimeWithTimeZone>,
    pub is_admin: bool,
    pub publish_notifications: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::crate_owners::Entity")]
    CrateOwners,
}

impl Related<super::crate_owners::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CrateOwners.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
