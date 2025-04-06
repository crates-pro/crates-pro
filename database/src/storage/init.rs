use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use std::time::Duration;
use tracing::log;

/// Create a database connection, then share connection with different conetxt
pub async fn database_connection(db_url: &str) -> Result<DatabaseConnection, DbErr> {
    log::info!("Connecting to database: {}", db_url);
    let mut opt = ConnectOptions::new(db_url);
    opt.max_connections(32)
        .min_connections(4)
        .acquire_timeout(Duration::from_secs(3))
        .connect_timeout(Duration::from_secs(3))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Debug);
    Database::connect(opt).await
}
