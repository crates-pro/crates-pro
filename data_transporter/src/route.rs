use std::cmp::Ordering;
use std::collections::HashSet;
//use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::time::Instant;

use crate::data_reader::DataReaderTrait;
use crate::db::DBHandler;
use crate::NameVersion;
use crate::Query;
use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use model::repo_sync_model;
use model::repo_sync_model::CrateType;
use repo_import::ImportDriver;
use sanitize_filename::sanitize;
use search::crates_search::RecommendCrate;
//use search::crates_search::RecommendCrate;
use search::crates_search::SearchModule;
use search::crates_search::SearchSortCriteria;
use serde::Deserialize;
use serde::Serialize;
use std::io::Cursor;
use std::io::Read;
//use std::time::Instant;
use semver::Version;
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
    nsfront: String,
    nsbehind: String,
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
    pub crate_name: String,
    pub description: String,
    pub dependencies: DependencyCount,
    pub dependents: DependentCount,
    pub cves: Vec<RustSec>,
    pub dep_cves: Vec<RustSec>,
    pub license: String,
    pub github_url: String,
    pub doc_url: String,
    pub versions: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyCount {
    pub direct: usize,
    pub indirect: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependentCount {
    pub direct: usize,
    pub indirect: usize,
}
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Hash)]
pub struct RustSec {
    pub id: String,
    pub cratename: String,
    pub patched: String,
    pub aliases: Vec<String>,
    pub small_desc: String,
}
impl ApiHandler {
    pub async fn new(reader: Box<dyn DataReaderTrait>) -> Self {
        Self { reader }
    }
    pub async fn get_direct_dep_for_graph(
        &self,
        nname: String,
        nversion: String,
    ) -> impl Responder {
        let name_and_version = nname + "/" + &nversion;
        let res = self
            .reader
            .get_direct_dependency_nodes(&name_and_version)
            .await
            .unwrap();
        HttpResponse::Ok().json(res)
    }
    pub async fn get_max_version(&self, versions: Vec<String>) -> Result<String, Box<dyn Error>> {
        let res = versions
            .into_iter()
            .max_by(|a, b| {
                // 提取主版本号（即+或-之前的部分）
                let a_base = a
                    .split_once('-')
                    .or_else(|| a.split_once('+'))
                    .map(|(a, _)| a)
                    .unwrap_or(a);
                let b_base = b
                    .split_once('-')
                    .or_else(|| b.split_once('+'))
                    .map(|(b, _)| b)
                    .unwrap_or(b);

                // 比较主版本号
                a_base
                    .split('.')
                    .zip(b_base.split('.'))
                    .map(|(a_part, b_part)| {
                        a_part
                            .parse::<i32>()
                            .unwrap()
                            .cmp(&b_part.parse::<i32>().unwrap())
                    })
                    // 如果所有部分都相等，则认为两个版本号相等
                    .find(|cmp_result| !cmp_result.is_eq())
                    .unwrap_or(Ordering::Equal)
            })
            .unwrap_or_else(|| "0.0.0".to_string());

        Ok(res)
    }
    pub async fn new_get_crates_front_info(
        &self,
        nname: String,
        nversion: String,
        nsfront: String,
        nsbehind: String,
    ) -> impl Responder {
        let mut name_and_version = nname.clone() + "/" + &nversion.clone();
        let namespace = nsfront.clone() + "/" + &nsbehind.clone();
        //println!("{}", name_and_version);
        if nversion == *"default" {
            //get max_version
            println!("enter default");
            let new_lib_versions = self
                .reader
                .new_get_lib_version(namespace.clone(), nname.clone())
                .await
                .unwrap();
            let new_app_versions = self
                .reader
                .new_get_app_version(namespace.clone(), nname.clone())
                .await
                .unwrap();
            let mut getnewversions = vec![];
            for version in new_lib_versions {
                getnewversions.push(version);
            }
            for version in new_app_versions {
                getnewversions.push(version);
            }

            let maxversion = self.get_max_version(getnewversions).await.unwrap();

            name_and_version = nname.clone() + "/" + &maxversion.clone();
        } //get dependency count
        println!("name_and_version:{}", name_and_version);
        #[allow(unused_variables)]
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
        let dbhandler = DBHandler { client };
        let qid = namespace.clone() + "/" + &nname + "/" + &nversion;
        let qres = dbhandler
            .query_crates_info_from_pg(&qid, nname.clone())
            .await
            .unwrap();
        if qres.is_empty() {
            let mut githuburl = self
                .reader
                .get_github_url(namespace.clone(), nname.clone())
                .await
                .unwrap();
            if githuburl == *"null" || githuburl == *"None" {
                githuburl = "".to_string();
            }
            let mut docurl = self
                .reader
                .get_doc_url(namespace.clone(), nname.clone())
                .await
                .unwrap();
            if docurl == *"null" || docurl == *"None" {
                docurl = "".to_string();
            }
            let direct_dependency_nodes = self
                .reader
                .new_get_direct_dependency_nodes(&namespace, &name_and_version)
                .await
                .unwrap();
            let direct_dependency_count = direct_dependency_nodes.len();
            println!(
                "finish get_direct_dependency_nodes:{}",
                direct_dependency_count
            ); //ok
            let all_dependency_nodes = self
                .reader
                .new_get_all_dependencies(namespace.clone(), name_and_version.clone())
                .await
                .unwrap();
            let mut indirect_dependency = vec![];
            for node in all_dependency_nodes.clone() {
                let mut dr = false;
                for node2 in direct_dependency_nodes.clone() {
                    let nv = node2.name.clone() + "/" + &node2.version.clone();
                    if node == nv {
                        dr = true;
                        break;
                    }
                }
                if !dr {
                    indirect_dependency.push(node);
                }
            }
            let indirect_dependency_count = indirect_dependency.len();
            //get dependent count
            let direct_dependent_nodes = self
                .reader
                .new_get_direct_dependent_nodes(&namespace, &name_and_version)
                .await
                .unwrap();
            let direct_dependent_count = direct_dependent_nodes.len();
            println!(
                "finish get_direct_dependent_nodes:{}",
                direct_dependent_count
            );
            let all_dependent_nodes = self
                .reader
                .new_get_all_dependents(namespace.clone(), name_and_version.clone())
                .await
                .unwrap();
            let mut indirect_dependent = vec![];
            for node in all_dependent_nodes {
                let mut dr = false;
                for node2 in direct_dependent_nodes.clone() {
                    let nv = node2.name.clone() + "/" + &node2.version.clone();
                    if node == nv {
                        dr = true;
                        break;
                    }
                }
                if !dr {
                    indirect_dependent.push(node);
                }
            }
            let indirect_dependent_count = indirect_dependent.len();

            let getcves = dbhandler
                .get_direct_rustsec(&nname, &nversion)
                .await
                .unwrap();
            let get_dependency_cves = dbhandler
                .get_dependency_rustsec(all_dependency_nodes.clone())
                .await
                .unwrap();
            let getlicense = dbhandler
                .get_license_by_name(&namespace, &nname)
                .await
                .unwrap();
            let lib_versions = self
                .reader
                .new_get_lib_version(namespace.clone(), nname.clone())
                .await
                .unwrap();
            let mut getversions = vec![];
            for version in lib_versions {
                getversions.push(version);
            }
            getversions.sort_by(|a, b| {
                let version_a = Version::parse(a);
                let version_b = Version::parse(b);

                match (version_a, version_b) {
                    (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                    (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => Ordering::Equal,
                }
            });
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
                cves: getcves,
                versions: getversions,
                license: getlicense[0].clone(),
                github_url: githuburl,
                doc_url: docurl,
                dep_cves: get_dependency_cves,
            };
            dbhandler
                .insert_crates_info_into_pg(
                    res.clone(),
                    namespace.clone(),
                    nname.clone(),
                    nversion.clone(),
                )
                .await
                .unwrap();
            HttpResponse::Ok().json(res)
        } else {
            HttpResponse::Ok().json(qres[0].clone())
        }
    }
    pub async fn get_cves(&self) -> impl Responder {
        //println!("enter get cve");
        #[allow(unused_variables)]
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
            NoTls,
        )
        .await
        .unwrap();
        //println!("connect client");
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let dbhd = DBHandler { client };
        let cves = dbhd.get_all_cvelist().await.unwrap();
        //println!("{:?}", cves);
        HttpResponse::Ok().json(cves)
    }
    pub async fn new_get_dependency(
        &self,
        name: String,
        version: String,
        nsfront: String,
        nsbehind: String,
    ) -> impl Responder {
        //let name_and_version = name.clone() + "/" + &version;
        let namespace = nsfront.clone() + "/" + &nsbehind.clone();
        let nameversion = name.clone() + "/" + &version.clone();
        println!("{} {}", namespace.clone(), nameversion.clone());
        let direct_nodes = self
            .reader
            .new_get_direct_dependency_nodes(&namespace, &nameversion)
            .await
            .unwrap();
        let getdirect_count = direct_nodes.len();
        let all_dependency_nodes = self
            .reader
            .new_get_all_dependencies(namespace.clone(), nameversion.clone())
            .await
            .unwrap();
        let mut indirect_dependency = vec![];
        for node in all_dependency_nodes {
            let mut dr = false;
            for node2 in direct_nodes.clone() {
                let nv = node2.name.clone() + "/" + &node2.version.clone();
                if node == nv {
                    dr = true;
                    break;
                }
            }
            if !dr {
                indirect_dependency.push(node);
            }
        }
        let indirect_dependency_count = indirect_dependency.len();
        let mut deps = vec![];
        for item in direct_nodes {
            let dep_count = self.reader.count_dependencies(item.clone()).await.unwrap();
            let dep = DependencyCrateInfo {
                crate_name: item.clone().name,
                version: item.clone().version,
                relation: "Direct".to_string(),
                license: "".to_string(),
                dependencies: dep_count,
            };
            deps.push(dep);
        }
        for item in indirect_dependency {
            let parts: Vec<&str> = item.split('/').collect();
            let newitem = NameVersion {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
            };
            let dep_count = self
                .reader
                .count_dependencies(newitem.clone())
                .await
                .unwrap();

            let dep = DependencyCrateInfo {
                crate_name: parts[0].to_string(),
                version: parts[1].to_string(),
                relation: "Indirect".to_string(),
                license: "".to_string(),
                dependencies: dep_count,
            };
            deps.push(dep);
        }

        let res_deps = DependencyInfo {
            direct_count: getdirect_count,
            indirect_count: indirect_dependency_count,
            data: deps,
        };
        HttpResponse::Ok().json(res_deps)
    }

    pub async fn new_get_dependent(
        &self,
        name: String,
        version: String,
        nsfront: String,
        nsbehind: String,
    ) -> impl Responder {
        //let name_and_version = name.clone() + "/" + &version;
        let namespace = nsfront.clone() + "/" + &nsbehind.clone();
        let nameversion = name.clone() + "/" + &version.clone();
        let direct_nodes = self
            .reader
            .new_get_direct_dependent_nodes(&namespace, &nameversion)
            .await
            .unwrap();
        let getdirect_count = direct_nodes.len();
        let all_dependent_nodes = self
            .reader
            .new_get_all_dependents(namespace.clone(), nameversion.clone())
            .await
            .unwrap();
        let mut indirect_dependent = vec![];
        for node in all_dependent_nodes {
            let mut dr = false;
            for node2 in direct_nodes.clone() {
                let nv = node2.name.clone() + "/" + &node2.version.clone();
                if node == nv {
                    dr = true;
                    break;
                }
            }
            if !dr {
                indirect_dependent.push(node);
            }
        }
        let indirect_dependent_count = indirect_dependent.len();
        let mut deps = vec![];
        let mut count1 = 0;
        for item in direct_nodes {
            let dep = DependentData {
                crate_name: item.clone().name,
                version: item.clone().version,
                relation: "Direct".to_string(),
            };
            deps.push(dep);
            count1 += 1;
            if count1 == 50 {
                break;
            }
        }
        let mut count2 = 0;
        for item in indirect_dependent {
            let parts: Vec<&str> = item.split('/').collect();
            let dep = DependentData {
                crate_name: parts[0].to_string(),
                version: parts[1].to_string(),
                relation: "Indirect".to_string(),
            };
            count2 += 1;
            if count2 == 50 {
                break;
            }
            deps.push(dep);
        }

        let res_deps = DependentInfo {
            direct_count: getdirect_count,
            indirect_count: indirect_dependent_count,
            data: deps,
        };
        HttpResponse::Ok().json(res_deps)
    }
    #[allow(clippy::vec_init_then_push)]
    pub async fn query_crates(&self, q: Query) -> impl Responder {
        //add yj's search module
        let name = q.query;
        println!("name: {}", name);
        let page = q.pagination.page;
        println!("page: {}", page);
        let per_page = q.pagination.per_page;
        println!("per_page: {}", per_page);
        //
        //let programs = self.reader.get_program_by_name(&name).await.unwrap();
        //
        //
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
        /*let start_time1 = Instant::now();
        let pre_search = search_prepare::SearchPrepare::new(&client).await;
        pre_search.prepare_tsv().await.unwrap();
        println!("prepare need time:{:?}", start_time1.elapsed());*/
        let start_time2 = Instant::now();
        let question = name.clone();
        let search_module = SearchModule::new(&client).await;
        let res = search_module
            .search_crate(&question, SearchSortCriteria::Relavance)
            .await
            .unwrap();
        println!("search need time:{:?}", start_time2.elapsed());
        //
        let mut seen = HashSet::new();
        let uniq_res: Vec<RecommendCrate> = res
            .into_iter()
            .filter(|x| seen.insert((x.name.clone(), x.namespace.clone())))
            .collect();
        println!("total programs: {}", uniq_res.len());
        let mut gettotal_page = uniq_res.len() / per_page;
        if uniq_res.is_empty() || uniq_res.len() % per_page != 0 {
            gettotal_page += 1;
        }
        let mut getitems = vec![];
        for i in (page - 1) * 20..(page - 1) * 20 + 20 {
            if i >= uniq_res.len() {
                break;
            }
            let mut mv = vec![];
            let program_name = uniq_res[i].clone().name;
            let getnamespace = uniq_res[i].clone().namespace;
            let parts: Vec<&str> = getnamespace.split('/').collect();
            let nsf = parts[0].to_string();
            let nsb = parts[1].to_string();
            //println!("{}", uniq_res[i].rank);
            //let endtime3 = starttime3.elapsed();
            //println!("get_max_version need time:{:?}", endtime3);
            /*if let Some(maxversion) = programs[i].clone().max_version {
                mv.push(maxversion);
            } else {
                mv.push("0.0.0".to_string());
            }*/
            mv.push(uniq_res[i].clone().max_version);
            //println!("maxversion {}", mv[0].clone());
            if mv[0].clone() == *"null" {
                mv[0] = "0.0.0".to_string();
            }
            let query_item = QueryItem {
                name: program_name.clone(),
                version: mv[0].clone(),
                date: "".to_string(),
                nsfront: nsf,
                nsbehind: nsb,
            };
            getitems.push(query_item);
        }
        let response = QueryCratesInfo {
            code: 200,
            message: "成功".to_string(),
            data: QueryData {
                total_page: gettotal_page,
                items: getitems,
            },
        };
        //println!("response {:?}", response);
        HttpResponse::Ok().json(response)
    }
    #[allow(dead_code)]
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
                        let _ = import_driver
                            .user_import_handler
                            .send_message(
                                &kafka_user_import_topic,
                                "",
                                &serde_json::to_string(&sent_payload).unwrap(),
                            )
                            .await;
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
                } /*else if Some("link") == field.name() {
                      // 处理 URL 链接
                      let mut url = String::new();
                      while let Some(chunk) = field.next().await {
                          url.push_str(&String::from_utf8(chunk.unwrap().to_vec()).unwrap());
                      }
                      println!("Received URL: {}", url);
                  }*/
            }
        }

        HttpResponse::Ok().json(analysis_result)
    }
}
