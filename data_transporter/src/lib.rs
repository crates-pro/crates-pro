mod data_packer;
mod data_reader;
mod db;
mod route;
mod transporter;

use std::sync::Arc;

use model::tugraph_model::UVersion;
use serde::{Deserialize, Serialize};
pub use transporter::Transporter;

use crate::data_reader::DataReader; // 确保导入你的 DataReader
use crate::route::ApiHandler;
use actix_multipart::Multipart;
use actix_web::{web, App, HttpServer};

pub async fn run_api_server(
    uri: &str,
    user: &str,
    password: &str,
    db: &str,
) -> std::io::Result<()> {
    let reader = DataReader::new(uri, user, password, db).await.unwrap();
    let api_handler = Arc::new(ApiHandler::new(Box::new(reader)).await);

    HttpServer::new(move || {
        let api_handler_clone = Arc::clone(&api_handler);
        App::new()
            .app_data(web::Data::from(api_handler_clone))
            .route(
                "/api/crates",
                web::get().to(|data: web::Data<Arc<ApiHandler>>| async move {
                    data.get_all_crates().await
                }),
            )
            .route(
                "/api/crates/{name}",
                web::get().to(
                    |data: web::Data<Arc<ApiHandler>>, name: web::Path<String>| async move {
                        data.get_crate_details(name.into_inner().into()).await
                    },
                ),
            )
            .route(
                "/api/crates",
                web::post().to(
                    |_data: web::Data<Arc<ApiHandler>>, payload: Multipart| async move {
                        ApiHandler::upload_crate(payload).await
                    },
                ),
            )
    })
    .bind("0.0.0.0:6888")?
    .run()
    .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NameVersion {
    pub name: String,
    pub version: String,
}

impl NameVersion {
    // 解析 "name/version" 格式的字符串
    pub fn from_string(name_version: &str) -> Option<Self> {
        let parts: Vec<&str> = name_version.split('/').collect();
        if parts.len() == 2 {
            Some(NameVersion {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version_base: UVersion,
    pub dependencies: Vec<NameVersion>,
}
