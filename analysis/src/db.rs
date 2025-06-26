use data_transporter::db::{db_connection_config_from_env, DBHandler};
use tokio_postgres::NoTls;

pub async fn get_dbhandler() -> DBHandler {
    let db_connection_config = db_connection_config_from_env();
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    DBHandler { client }
}
