mod data_packer;
mod data_reader;
mod db;
mod route;
mod transporter;

use std::sync::Arc;

use model::tugraph_model::UVersion;
use search::search_prepare;
use serde::{Deserialize, Serialize};
use tokio_postgres::NoTls;
pub use transporter::Transporter;

use crate::data_reader::DataReader; // 确保导入你的 DataReader
use crate::route::ApiHandler;

use actix_multipart::Multipart;
use actix_web::{web, App, HttpResponse, HttpServer};
#[derive(Deserialize, Debug)]
pub struct Query {
    query: String,
    pagination: Pagination,
}
#[derive(Deserialize, Debug)]
pub struct Pagination {
    page: usize,
    per_page: usize,
}

pub async fn run_api_server(
    uri: &str,
    user: &str,
    password: &str,
    db: &str,
) -> std::io::Result<()> {
    tracing::info!("Start run_api_server");
    let reader = DataReader::new(uri, user, password, db).await.unwrap();
    let api_handler = Arc::new(ApiHandler::new(Box::new(reader)).await);
    let (client, connection) = tokio_postgres::connect(
        "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
        NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let pre_search = search_prepare::SearchPrepare::new(&client).await;
    pre_search.prepare_tsv().await.unwrap();
    HttpServer::new(move || {
        let api_handler_clone = Arc::clone(&api_handler);
        tracing::info!("start route");
        App::new()
            .app_data(web::Data::new(api_handler_clone))
            .route(
                "/api/crates",
                web::get().to(|data: web::Data<Arc<ApiHandler>>| async move {
                    data.get_all_crates().await
                }),
            )
            .route(
                "/api/crates/{cratename}",
                web::get().to(
                    |data: web::Data<Arc<ApiHandler>>, name: web::Path<String>| async move {
                        println!("2");
                        data.get_crate_details(name.into_inner().into()).await
                    },
                ),
            )
            .route(
                "/api/cvelist",
                web::get().to(
                    |data: web::Data<Arc<ApiHandler>>| async move {
                        //println!("get cves");
                        data.get_cves().await
                    },
                ),
            )
            .route(
                "/api/submit",
                web::post().to(
                    |_data: web::Data<Arc<ApiHandler>>, payload: Multipart| async move {
                        println!("submit!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
                        ApiHandler::upload_crate(payload).await
                    },
                ),
            )
            .route("/api/search", web::post().to(
                |data: web::Data<Arc<ApiHandler>>,payload: web::Json<Query>| async move{
                    //println!("3");
                    let query = payload.into_inner();
                    //println!("query {:?}",query);
                    data.query_crates(query).await
            },),)
            .route("/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies", 
            web::get().to(|data:web::Data<Arc<ApiHandler>>,path: web::Path<(String, String,String,String)>|async move{
                println!("2");
                let (nsfront,nsbehind,cratename, version) = path.into_inner();
                data.new_get_dependency(cratename,version,nsfront,nsbehind).await
            }))
            .route("/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies/graph", 
            web::get().to(|_data:web::Data<Arc<ApiHandler>>,_path: web::Path<(String, String,String,String)>|async move{
                println!("2");
                //let (nsfront,nsbehind,cratename, version) = path.into_inner();
                //data.new_get_dependency(cratename,version,nsfront,nsbehind).await
                HttpResponse::Ok().json(())
            }))
            .route("/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependents", 
            web::get().to(|data:web::Data<Arc<ApiHandler>>,path: web::Path<(String, String,String,String)>|async move{
                let (nsfront,nsbehind,cratename, version) = path.into_inner();
                data.new_get_dependent(cratename,version,nsfront,nsbehind).await
            }))
            .route("/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}", 
            web::get().to(|data:web::Data<Arc<ApiHandler>>,path: web::Path<(String, String,String,String)>|async move{
                println!("1");
                let (nsfront,nsbehind,cratename, version) = path.into_inner();
                data.new_get_crates_front_info(cratename,version,nsfront,nsbehind).await
            }))
            .route("/api/graph/{cratename}/{version}/direct", 
            web::get().to(|data:web::Data<Arc<ApiHandler>>,path: web::Path<(String,String)>|async move{
                println!("enter get graph");
                let (cratename, version) = path.into_inner();
                data.get_direct_dep_for_graph(cratename,version).await
            }))
    })
    .bind("0.0.0.0:6888")?
    .run()
    .await
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
