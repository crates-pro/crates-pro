pub use sea_orm_migration::prelude::*;

mod m20250331_093145_init_crates_pro;
mod m20250401_100000_add_performance_indices;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250331_093145_init_crates_pro::Migration),
            Box::new(m20250401_100000_add_performance_indices::Migration),
        ]
    }
}
