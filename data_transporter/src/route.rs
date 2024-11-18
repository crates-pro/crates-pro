use std::env;

use crate::data_reader::DataReaderTrait;
use crate::db::DBHandler;
use crate::Query;
use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use model::repo_sync_model;
use model::repo_sync_model::CrateType;
use repo_import::ImportDriver;
use sanitize_filename::sanitize;
use serde::Deserialize;
use serde::Serialize;
use std::io::Cursor;
use std::io::Read;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_postgres::NoTls;
use zip::ZipArchive;
pub struct ApiHandler {
    reader: Box<dyn DataReaderTrait>,
}
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct QueryCratesInfo {
    code: u32,
    message: String,
    data: QueryData,
}
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct QueryData {
    total_page: usize,
    items: Vec<QueryItem>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct QueryItem {
    name: String,
    version: String,
    date: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyCrateInfo {
    crate_name: String,
    version: String,
    relation: String,
    license: String,
    dependencies: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyInfo {
    direct_count: usize,
    indirect_count: usize,
    data: Vec<DependencyCrateInfo>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependentInfo {
    direct_count: usize,
    indirect_count: usize,
    data: Vec<DependentData>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependentData {
    crate_name: String,
    version: String,
    relation: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Crateinfo {
    crate_name: String,
    description: String,
    dependencies: DependencyCount,
    dependents: DependentCount,
    cves: Vec<String>,
    versions: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyCount {
    direct: usize,
    indirect: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependentCount {
    direct: usize,
    indirect: usize,
}
impl ApiHandler {
    pub async fn new(reader: Box<dyn DataReaderTrait>) -> Self {
        Self { reader }
    }
    pub async fn get_crates_front_info(
        &self,
        name: web::Path<String>,
        version: web::Path<String>,
    ) -> impl Responder {
        let nname: String = name.into_inner().into();
        let nversion: String = version.into_inner().into();
        let mut name_and_version = nname.clone() + "/" + &nversion.clone();
        if nversion == "".to_string() {
            //get max_version
            let max_version = self.reader.get_max_version(nname.clone()).await.unwrap();
            name_and_version = nname.clone() + "/" + &max_version;
        } //get dependency count
        let mut all_dependency_nodes = self
            .reader
            .get_direct_dependency_nodes(&name_and_version)
            .await
            .unwrap();
        let direct_dependency_count = all_dependency_nodes.len();
        for node in all_dependency_nodes.clone() {
            let nodes = self
                .reader
                .get_indirect_dependency_nodes(node)
                .await
                .unwrap();
            for indirect_node in nodes {
                all_dependency_nodes.push(indirect_node);
            }
        }
        let indirect_dependency_count = all_dependency_nodes.len() - direct_dependency_count;
        //get dependent count
        let mut all_dependent_nodes = self
            .reader
            .get_direct_dependent_nodes(&name_and_version)
            .await
            .unwrap();
        let direct_dependent_count = all_dependent_nodes.len();
        for node in all_dependent_nodes.clone() {
            let nodes = self
                .reader
                .get_indirect_dependent_nodes(node)
                .await
                .unwrap();
            for indirect_node in nodes {
                all_dependent_nodes.push(indirect_node);
            }
        }
        let indirect_dependent_count = all_dependent_nodes.len() - direct_dependent_count;
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
            NoTls,
        )
        .await
        .unwrap();
        let dbhandler = DBHandler { client };
        let cves = dbhandler.get_cve_by_cratename(&nname).await.unwrap();
        let lib_versions = self.reader.get_lib_version(nname.clone()).await.unwrap();
        let app_versions = self.reader.get_app_version(nname.clone()).await.unwrap();
        let mut versions = vec![];
        for version in lib_versions {
            versions.push(version);
        }
        for version in app_versions {
            versions.push(version);
        }
        let dcy_count = DependencyCount {
            direct: direct_dependency_count,
            indirect: indirect_dependency_count,
        };
        let dt_count = DependentCount {
            direct: direct_dependent_count,
            indirect: indirect_dependent_count,
        };
        let res = Crateinfo {
            crate_name: nname.clone(),
            description: "".to_string(),
            dependencies: dcy_count,
            dependents: dt_count,
            cves: cves,
            versions: versions,
        };
        HttpResponse::Ok().json(res)
    }
    pub async fn get_cves(&self) -> impl Responder {
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
            NoTls,
        )
        .await
        .unwrap();
        let dbhd = DBHandler { client };
        let cves = dbhd.get_all_cvelist().await.unwrap();
        HttpResponse::Ok().json(cves)
    }
    pub async fn get_dependency(
        &self,
        name: web::Path<String>,
        version: web::Path<String>,
    ) -> impl Responder {
        let name_and_version = name.into_inner() + "/" + &version.into_inner();
        let mut all_nodes = self
            .reader
            .get_direct_dependency_nodes(&name_and_version)
            .await
            .unwrap();
        let direct_count = all_nodes.len();
        for node in all_nodes.clone() {
            let nodes = self
                .reader
                .get_indirect_dependency_nodes(node)
                .await
                .unwrap();
            for indirect_node in nodes {
                all_nodes.push(indirect_node);
            }
        }
        let indirect_count = all_nodes.len() - direct_count;
        let mut deps = vec![];
        for i in 0..direct_count - 1 {
            let dep_count = self
                .reader
                .count_dependencies(all_nodes[i].clone())
                .await
                .unwrap();
            let dep = DependencyCrateInfo {
                crate_name: all_nodes[i].clone().name,
                version: all_nodes[i].clone().version,
                relation: "Direct".to_string(),
                license: "".to_string(),
                dependencies: dep_count,
            };
            deps.push(dep);
        }
        for i in direct_count..all_nodes.len() - 1 {
            let dep_count = self
                .reader
                .count_dependencies(all_nodes[i].clone())
                .await
                .unwrap();
            let dep = DependencyCrateInfo {
                crate_name: all_nodes[i].clone().name,
                version: all_nodes[i].clone().version,
                relation: "Indirect".to_string(),
                license: "".to_string(),
                dependencies: dep_count,
            };
            deps.push(dep);
        }
        let res_deps = DependencyInfo {
            direct_count: direct_count,
            indirect_count: indirect_count,
            data: deps,
        };
        HttpResponse::Ok().json(res_deps)
    }
    pub async fn get_dependent(
        &self,
        name: web::Path<String>,
        version: web::Path<String>,
    ) -> impl Responder {
        let name_and_version = name.into_inner() + "/" + &version.into_inner();
        let mut all_nodes = self
            .reader
            .get_direct_dependent_nodes(&name_and_version)
            .await
            .unwrap();
        let direct_count = all_nodes.len();
        for node in all_nodes.clone() {
            let nodes = self
                .reader
                .get_indirect_dependent_nodes(node)
                .await
                .unwrap();
            for indirect_node in nodes {
                all_nodes.push(indirect_node);
            }
        }
        let indirect_count = all_nodes.len() - direct_count;
        let mut deps = vec![];
        for i in 0..direct_count - 1 {
            let dep = DependentData {
                crate_name: all_nodes[i].clone().name,
                version: all_nodes[i].clone().version,
                relation: "Direct".to_string(),
            };
            deps.push(dep);
            if i == 49 {
                break;
            }
        }
        for i in direct_count..all_nodes.len() - 1 {
            let dep = DependentData {
                crate_name: all_nodes[i].clone().name,
                version: all_nodes[i].clone().version,
                relation: "Indirect".to_string(),
            };
            deps.push(dep);
            if i - direct_count == 49 {
                break;
            }
        }
        let res_deps = DependentInfo {
            direct_count: direct_count,
            indirect_count: indirect_count,
            data: deps,
        };
        HttpResponse::Ok().json(res_deps)
    }
    pub async fn query_crates(&self, q: Query) -> impl Responder {
        let name = q.query;
        let page = q.pagination.page;
        let per_page = q.pagination.per_page;
        let programs = self.reader.get_program_by_name(&name).await.unwrap();
        let total_page = programs.len() / per_page;
        let mut items = vec![];
        for i in page * 20..page * 20 + 19 {
            if i >= programs.len() {
                break;
            }
            let query_item = QueryItem {
                name: programs[i].clone().name,
                version: programs[i].clone().max_version.unwrap(),
                date: "".to_string(),
            };
            items.push(query_item);
        }
        let response = QueryCratesInfo {
            code: 200,
            message: "成功".to_string(),
            data: QueryData {
                total_page: total_page,
                items: items,
            },
        };
        HttpResponse::Ok().json(response)
    }
    pub async fn get_all_crates_id(&self) -> impl Responder {
        tracing::info!("get all crates func run");
        let program_ids = { self.reader.get_all_programs_id() }.await;
        tracing::info!("finish get all crates func");
        //for id in program_ids.clone() {
        //    tracing::info!("program id: {}", id);
        //}
        HttpResponse::Ok().json(program_ids) // 返回 JSON 格式
    }

    pub async fn get_all_crates(&self) -> impl Responder {
        tracing::info!("get all crates func run");
        let ids = self.reader.get_all_programs_id().await;

        let mut programs = vec![];
        for id in &ids {
            let program = self.reader.get_program(id).await.unwrap();
            programs.push(program);
        }

        //let program_ids = { self.reader.get_all_programs_id() }.await;
        tracing::info!("finish get all crates func");
        //for id in program_ids.clone() {
        //    tracing::info!("program id: {}", id);
        //}
        HttpResponse::Ok().json(programs) // 返回 JSON 格式
    }

    pub async fn get_crate_details(&self, crate_name: web::Path<String>) -> impl Responder {
        match self.reader.get_program(&crate_name).await {
            Ok(program) => {
                match self.reader.get_type(&crate_name).await {
                    Ok((uprogram, islib)) => {
                        match self.reader.get_versions(&crate_name, islib).await {
                            Ok(versions) => {
                                HttpResponse::Ok().json((program, uprogram, versions))
                                // 返回 JSON 格式
                            }
                            Err(_) => {
                                HttpResponse::InternalServerError().body("Failed to get versions.")
                            }
                        }
                    }
                    Err(_) => HttpResponse::InternalServerError().body("Failed to get type."),
                }
            }
            Err(_) => HttpResponse::NotFound().body("Crate not found."),
        }
    }

    pub async fn upload_crate(mut payload: Multipart) -> impl Responder {
        use futures_util::StreamExt as _;
        let analysis_result = String::new();
        while let Some(Ok(mut field)) = payload.next().await {
            if let Some(content_disposition) = field.content_disposition() {
                if let Some(filename) = content_disposition.get_filename() {
                    let sanitized_filename = sanitize(filename);
                    if sanitized_filename.ends_with(".zip") {
                        let zip_filepath = format!("target/zip/upload/{}", sanitized_filename);
                        let _ = tokio::fs::create_dir_all("target/zip/upload/").await;
                        let mut f = tokio::fs::File::create(&zip_filepath).await.unwrap();
                        while let Some(chunk) = field.next().await {
                            let data = chunk.unwrap();
                            f.write_all(&data).await.unwrap();
                        }
                        let parts: Vec<&str> = sanitized_filename.split('.').collect();
                        let mut filename = "".to_string();
                        if parts.len() >= 2 {
                            filename = parts[0].to_string();
                            println!("filename without zip: {}", filename);
                        }
                        let mut zip_file = tokio::fs::File::open(&zip_filepath).await.unwrap();
                        let mut buffer = Vec::new();
                        zip_file.read_to_end(&mut buffer).await.unwrap();
                        let reader = Cursor::new(buffer.clone());
                        let mut archive = ZipArchive::new(reader).unwrap();
                        for i in 0..archive.len() {
                            let mut file = archive.by_index(i).unwrap();
                            let outpath = match file.enclosed_name() {
                                Some(path) => {
                                    format!("target/www/uploads/{}/{}", filename, path.display())
                                }
                                None => continue,
                            };

                            if file.name().ends_with('/') {
                                // This is a directory, create it
                                tokio::fs::create_dir_all(&outpath).await.unwrap();
                            } else {
                                // Ensure the parent directory exists
                                if let Some(parent) = std::path::Path::new(&outpath).parent() {
                                    if !parent.exists() {
                                        tokio::fs::create_dir_all(&parent).await.unwrap();
                                    }
                                }

                                // Write the file
                                let mut outfile = tokio::fs::File::create(&outpath).await.unwrap();
                                while let Ok(bytes_read) = file.read(&mut buffer) {
                                    if bytes_read == 0 {
                                        break;
                                    }
                                    outfile.write_all(&buffer[..bytes_read]).await.unwrap();
                                }
                            }
                        }
                        //send message
                        let send_url = format!("target/www/uploads/{}", filename);
                        let sent_payload = repo_sync_model::Model {
                            id: 0,
                            crate_name: filename,
                            github_url: None,
                            mega_url: send_url,
                            crate_type: CrateType::Lib,
                            status: model::repo_sync_model::RepoSyncStatus::Syncing,
                            err_message: None,
                        };
                        let kafka_user_import_topic = env::var("KAFKA_USER_IMPORT_TOPIC").unwrap();
                        let import_driver = ImportDriver::new(false).await;
                        let _ = import_driver.user_import_handler.send_message(
                            &kafka_user_import_topic,
                            "",
                            &serde_json::to_string(&sent_payload).unwrap(),
                        );
                        break;
                    } else {
                        let filepath =
                            format!("/home/rust/output/www/uploads/{}", sanitized_filename);
                        let mut f = tokio::fs::File::create(&filepath).await.unwrap();

                        while let Some(chunk) = field.next().await {
                            let data = chunk.unwrap();
                            f.write_all(&data).await.unwrap();
                        }
                        break;
                    }
                    // analyze
                } else if Some("link") == field.name() {
                    // 处理 URL 链接
                    let mut url = String::new();
                    while let Some(chunk) = field.next().await {
                        url.push_str(&String::from_utf8(chunk.unwrap().to_vec()).unwrap());
                    }
                    println!("Received URL: {}", url);
                }
            }
        }

        HttpResponse::Ok().json(analysis_result)
    }
}
