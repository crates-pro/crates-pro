use std::cmp::Ordering;
use std::collections::HashSet;
#[allow(unused_imports)]
use std::env;
use std::error::Error;
use std::time::Instant;

use crate::data_reader::{DataReader, DataReaderTrait};
use crate::db::{db_connection_config_from_env, db_cratesio_connection_config_from_env, DBHandler};
use crate::{get_tugraph_api_handler, NameVersion, Userinfo};
use crate::{Query, VersionInfo};
use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
//use model::repo_sync_model;
//use model::repo_sync_model::CrateType;
use model::tugraph_model::{Program, UProgram};
//use repo_import::ImportDriver;
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
use utoipa::ToSchema;
use zip::ZipArchive;
pub struct ApiHandler {
    reader: DataReader,
}
impl ApiHandler {
    pub async fn new(reader: DataReader) -> Self {
        Self { reader }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, ToSchema)]
pub struct QueryCratesInfo {
    code: u32,
    message: String,
    data: QueryData,
}
#[derive(Serialize, Deserialize, Debug, Default, Clone, ToSchema)]
pub struct QueryData {
    total_page: usize,
    items: Vec<QueryItem>,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct QueryItem {
    name: String,
    version: String,
    date: String,
    nsfront: String,
    nsbehind: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependencyCrateInfo {
    pub crate_name: String,
    pub version: String,
    pub relation: String,
    pub license: String,
    pub dependencies: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependencyInfo {
    pub direct_count: usize,
    pub indirect_count: usize,
    pub data: Vec<DependencyCrateInfo>,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependentInfo {
    pub direct_count: usize,
    pub indirect_count: usize,
    pub data: Vec<DependentData>,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependentData {
    pub crate_name: String,
    pub version: String,
    pub relation: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct Crateinfo {
    pub crate_name: String,
    pub description: String,
    pub dependencies: DependencyCount,
    pub dependents: DependentCount,
    pub cves: Vec<NewRustsec>,
    pub dep_cves: Vec<NewRustsec>,
    pub license: String,
    pub github_url: String,
    pub doc_url: String,
    pub versions: Vec<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependencyCount {
    pub direct: usize,
    pub indirect: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct DependentCount {
    pub direct: usize,
    pub indirect: usize,
}
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Hash, ToSchema)]
pub struct RustSec {
    pub id: String,
    pub cratename: String,
    pub patched: String,
    pub aliases: Vec<String>,
    pub small_desc: String,
}
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Hash, ToSchema)]
pub struct NewRustsec {
    pub id: String,
    pub subtitle: String,
    pub reported: String,
    pub issued: String,
    pub package: String,
    pub ttype: String,
    pub keywords: String,
    pub aliases: String,
    pub reference: String,
    pub patched: String,
    pub unaffected: String,
    pub description: String,
    pub url: String,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct Deptree {
    pub name_and_version: String,
    pub cve_count: usize,
    #[schema(value_type = Vec<Deptree>)]
    pub direct_dependency: Vec<Deptree>,
}
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct Versionpage {
    pub version: String,
    pub updated_at: String,
    pub downloads: String,
    pub dependents: usize,
}

/// 获取cve信息
#[utoipa::path(
    get,
    path = "/api/cvelist",
    responses(
        (status = 200, description = "成功获取crate信息", body = crate::db::Allcve),
        (status = 404, description = "未找到crate信息")
    ),
    tag = "security"
)]
pub async fn get_cves() -> impl Responder {
    let _handler = get_tugraph_api_handler().await;
    //println!("enter get cve");
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
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

/// 获取所有crates
#[utoipa::path(
    get,
    path = "/api/crates",
    responses(
        (status = 200, description = "成功获取所有crate的id", body = Vec<model::tugraph_model::Program>),
        (status = 404, description = "未找到crate id")
    ),
    tag = "crates"
)]
pub async fn get_all_crates() -> impl Responder {
    tracing::info!("get all crates func run");
    let handler = get_tugraph_api_handler().await;
    let ids = handler.reader.get_all_programs_id().await;

    let mut programs = vec![];
    for id in &ids {
        let program = handler.reader.get_program(id).await.unwrap();
        programs.push(program);
    }

    //let program_ids = { self.reader.get_all_programs_id() }.await;
    tracing::info!("finish get all crates func");
    //for id in program_ids.clone() {
    //    tracing::info!("program id: {}", id);
    //}
    HttpResponse::Ok().json(programs) // 返回 JSON 格式
}

/// 获取crate详细信息,ok
#[utoipa::path(
    get,
    path = "/api/crates/{cratename}",
    params(
        ("cratename" = String, Path, description = "crate 名称")
    ),
    responses(
        (status = 200, description = "成功获取crate详细信息", body= (Program, UProgram, Vec<VersionInfo>)),
        (status = 404, description = "未找到crate"),
        (status = 500, description = "服务器内部错误")
    ),
    tag = "crates"
)]
pub async fn get_crate_details(crate_name: web::Path<String>) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    match handler.reader.get_program(&crate_name).await {
        Ok(program) => {
            match handler.reader.get_type(&crate_name).await {
                Ok((uprogram, islib)) => {
                    match handler.reader.get_versions(&crate_name, islib).await {
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

/// 获取版本页面信息,ok
#[utoipa::path(
    get,
    path = "/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/versions",
    params(
        ("nsfront" = String, Path, description = "命名空间前缀"),
        ("nsbehind" = String, Path, description = "命名空间后缀"),
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")//TODO:?
    ),
    responses(
        (status = 200, description = "成功获取版本信息", body = Vec<Versionpage>),
        (status = 404, description = "未找到版本信息")
    ),
    tag = "versions"
)]
pub async fn get_version_page(
    nsfront: String,
    nsbehind: String,
    nname: String,
    _nversion: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let db_connection_config = db_connection_config_from_env();
    let _db_cratesio_connection_config = db_cratesio_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    #[allow(unused_variables)]
    let (client2, connection2) = tokio_postgres::connect(&_db_cratesio_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection2.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler2 = DBHandler { client: client2 };
    tracing::info!("finish connect cratespro");
    let res = dbhandler
        .get_version_from_pg(nsfront.clone(), nsbehind.clone(), nname.clone())
        .await
        .unwrap();
    tracing::info!("finish get version from pg");
    if res.is_empty() {
        tracing::info!("res is empty");
        let namespace = nsfront.clone() + "/" + &nsbehind;
        let all_versions = handler
            .reader
            .new_get_lib_version(namespace.clone(), nname.clone())
            .await
            .unwrap();
        tracing::info!("finish get all versions");
        let mut getversions = vec![];
        for version in all_versions {
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
        let mut every_version = vec![];
        for version in getversions {
            let name_and_version = nname.clone() + "/" + &version.clone();
            let all_dts = handler
                .reader
                .new_get_all_dependents(namespace.clone(), name_and_version.clone())
                .await
                .unwrap();
            tracing::info!("finish get all dependents");
            let res = dbhandler2
                .get_dump_from_cratesio_pg(nname.clone(), version.clone())
                .await
                .unwrap();
            tracing::info!("finish get dump from pg");
            if !res.is_empty() {
                let parts: Vec<&str> = res.split("/").collect();
                if parts.len() == 2 {
                    let versionpage = Versionpage {
                        version,
                        dependents: all_dts.len(),
                        updated_at: parts[0].to_string(),
                        downloads: parts[1].to_string(),
                    };
                    every_version.push(versionpage);
                }
            }
        }
        dbhandler
            .insert_version_into_pg(
                nsbehind.clone(),
                nsfront.clone(),
                nname.clone(),
                every_version.clone(),
            )
            .await
            .unwrap();
        HttpResponse::Ok().json(every_version)
    } else {
        let all_version = res[0].clone();
        let mut every_version = vec![];
        let parts1: Vec<&str> = all_version.split('/').collect();
        for part in parts1 {
            let tmp_version = part.to_string();
            let parts2: Vec<&str> = tmp_version.split('|').collect();
            let res_version = parts2[0].to_string();
            let res_updated = parts2[1].to_string();
            let res_downloads = parts2[2].to_string();
            let res_dependents = parts2[3].to_string();
            let res_versionpage = Versionpage {
                version: res_version.clone(),
                dependents: res_dependents.parse::<usize>().unwrap(),
                updated_at: res_updated,
                downloads: res_downloads,
            };
            every_version.push(res_versionpage);
        }
        HttpResponse::Ok().json(every_version)
    }
}

/// 获取依赖图
#[utoipa::path(
    get,
    path = "/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies/graphpage",
    params(
        ("nsfront" = String, Path, description = "命名空间前缀"),
        ("nsbehind" = String, Path, description = "命名空间后缀"),
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")
    ),
    responses(
        (status = 200, description = "成功获取依赖图", body = Deptree),
        (status = 404, description = "未找到依赖图")
    ),
    tag = "dependencies"
)]
pub async fn get_graph(
    nsfront: String,
    nsbehind: String,
    nname: String,
    nversion: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    let res = dbhandler
        .get_graph_from_pg(
            nsfront.clone(),
            nsbehind.clone(),
            nname.clone(),
            nversion.clone(),
        )
        .await
        .unwrap();
    if res.is_empty() {
        println!("first time");
        let nav = nname.clone() + "/" + &nversion;
        let rustcve = dbhandler
            .get_direct_rustsec(&nname, &nversion)
            .await
            .unwrap();
        let mut res = Deptree {
            name_and_version: nav.clone(),
            cve_count: rustcve.len(),
            direct_dependency: Vec::new(),
        };
        let mut visited = HashSet::new();
        visited.insert(nav.clone());
        handler
            .reader
            .build_graph(&mut res, &mut visited)
            .await
            .unwrap();
        let graph = serde_json::to_string(&res).unwrap();
        dbhandler
            .insert_graph_into_pg(
                nsfront.clone(),
                nsbehind.clone(),
                nname.clone(),
                nversion.clone(),
                graph.clone(),
            )
            .await
            .unwrap();
        HttpResponse::Ok().json(res)
    } else {
        println!("second time");
        let res_tree: Deptree = serde_json::from_str(&res[0]).unwrap();
        HttpResponse::Ok().json(res_tree)
    }
}

/// 获取直接依赖关系图,ok
#[utoipa::path(
    get,
    path = "/api/graph/{cratename}/{version}/direct",
    params(
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")
    ),
    responses(
        (status = 200, description = "成功获取依赖关系图", body = Vec<NameVersion>),
        (status = 404, description = "未找到依赖关系图")
    ),
    tag = "dependencies"
)]
pub async fn get_direct_dep_for_graph(nname: String, nversion: String) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let name_and_version = nname + "/" + &nversion;
    let res = handler
        .reader
        .get_direct_dependency_nodes(&name_and_version)
        .await
        .unwrap();
    HttpResponse::Ok().json(res)
}

pub async fn get_max_version(versions: Vec<String>) -> Result<String, Box<dyn Error>> {
    let _handler = get_tugraph_api_handler().await;
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

/// 获取crate主页面
#[utoipa::path(
    get,
    path = "/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}",
    params(
        ("nsfront" = String, Path, description = "命名空间前缀"),
        ("nsbehind" = String, Path, description = "命名空间后缀"), 
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")
    ),
    responses(
        (status = 200, description = "成功获取crate信息"),
        (status = 404, description = "未找到相关crate信息")
    ),
    tag = "crates"
)]
pub async fn new_get_crates_front_info(
    nname: String,
    nversion: String,
    nsfront: String,
    nsbehind: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let mut name_and_version = nname.clone() + "/" + &nversion.clone();
    let namespace = nsfront.clone() + "/" + &nsbehind.clone();
    //println!("{}", name_and_version);
    if nversion == *"default" {
        //get max_version
        println!("enter default");
        let new_lib_versions = handler
            .reader
            .new_get_lib_version(namespace.clone(), nname.clone())
            .await
            .unwrap();
        let new_app_versions = handler
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

        let maxversion = get_max_version(getnewversions).await.unwrap();

        name_and_version = nname.clone() + "/" + &maxversion.clone();
    } //get dependency count
    tracing::info!("name_and_version:{}", name_and_version);
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    tracing::info!("finish connect pg");
    let dbhandler = DBHandler { client };
    let qid = namespace.clone() + "/" + &nname + "/" + &nversion;
    let qres = dbhandler
        .query_crates_info_from_pg(&qid, nname.clone())
        .await
        .unwrap();
    tracing::info!("finish query crates from pg");
    if qres.is_empty() {
        tracing::info!("qres is empty");
        let mut githuburl = handler
            .reader
            .get_github_url(namespace.clone(), nname.clone())
            .await
            .unwrap();
        if githuburl == *"null" || githuburl == *"None" {
            githuburl = "".to_string();
        }
        let mut docurl = handler
            .reader
            .get_doc_url(namespace.clone(), nname.clone())
            .await
            .unwrap();
        if docurl == *"null" || docurl == *"None" {
            docurl = "".to_string();
        }
        let direct_dependency_nodes = handler
            .reader
            .new_get_direct_dependency_nodes(&namespace, &name_and_version)
            .await
            .unwrap();
        let direct_dependency_count = direct_dependency_nodes.len();
        tracing::info!(
            "finish get_direct_dependency_nodes:{}",
            direct_dependency_count
        ); //ok
        let all_dependency_nodes = handler
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
        let direct_dependent_nodes = handler
            .reader
            .new_get_direct_dependent_nodes(&namespace, &name_and_version)
            .await
            .unwrap();
        let direct_dependent_count = direct_dependent_nodes.len();
        tracing::info!(
            "finish get_direct_dependent_nodes:{}",
            direct_dependent_count
        );
        /*let all_dependent_nodes = self
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
        let indirect_dependent_count = indirect_dependent.len();*/

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
        let lib_versions = handler
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
            indirect: 0,
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

pub async fn new_get_dependency(
    name: String,
    version: String,
    nsfront: String,
    nsbehind: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    //let name_and_version = name.clone() + "/" + &version;
    let namespace = nsfront.clone() + "/" + &nsbehind.clone();
    let nameversion = name.clone() + "/" + &version.clone();
    println!("{} {}", namespace.clone(), nameversion.clone());
    let direct_nodes = handler
        .reader
        .new_get_direct_dependency_nodes(&namespace, &nameversion)
        .await
        .unwrap();
    let getdirect_count = direct_nodes.len();
    let all_dependency_nodes = handler
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
        let dep_count = handler
            .reader
            .count_dependencies(item.clone())
            .await
            .unwrap();
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
        let dep_count = handler
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
///获取依赖关系
#[utoipa::path(
    get,
    path = "/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies",
    params(
        ("nsfront" = String, Path, description = "命名空间前缀"),
        ("nsbehind" = String, Path, description = "命名空间后缀"), 
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")
    ),
    responses(
        (status = 200, description = "成功获取依赖关系", body = DependencyInfo),
        (status = 404, description = "未找到依赖关系")
    ),
    tag = "dependencies"
)]
pub async fn dependency_cache(
    name: String,
    version: String,
    nsfront: String,
    nsbehind: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    let res = dbhandler
        .get_dependency_from_pg(
            nsfront.clone(),
            nsbehind.clone(),
            name.clone(),
            version.clone(),
        )
        .await
        .unwrap();
    if res.is_empty() {
        let namespace = nsfront.clone() + "/" + &nsbehind.clone();
        let nameversion = name.clone() + "/" + &version.clone();
        println!("{} {}", namespace.clone(), nameversion.clone());
        let direct_nodes = handler
            .reader
            .new_get_direct_dependency_nodes(&namespace, &nameversion)
            .await
            .unwrap();
        let getdirect_count = direct_nodes.len();
        let all_dependency_nodes = handler
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
            let dep_count = handler
                .reader
                .count_dependencies(item.clone())
                .await
                .unwrap();
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
            let dep_count = handler
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
        dbhandler
            .insert_dependency_into_pg(
                nsfront.clone(),
                nsbehind.clone(),
                name.clone(),
                version.clone(),
                res_deps.clone(),
            )
            .await
            .unwrap();
        HttpResponse::Ok().json(res_deps.clone())
    } else {
        HttpResponse::Ok().json(res[0].clone())
    }
}

pub async fn new_get_dependent(
    name: String,
    version: String,
    nsfront: String,
    nsbehind: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    //let name_and_version = name.clone() + "/" + &version;
    let namespace = nsfront.clone() + "/" + &nsbehind.clone();
    let nameversion = name.clone() + "/" + &version.clone();
    let direct_nodes = handler
        .reader
        .new_get_direct_dependent_nodes(&namespace, &nameversion)
        .await
        .unwrap();
    let getdirect_count = direct_nodes.len();
    // let all_dependent_nodes = self
    //     .reader
    //     .new_get_all_dependents(namespace.clone(), nameversion.clone())
    //     .await
    //     .unwrap();
    /*let mut indirect_dependent = vec![];
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
    let indirect_dependent_count = indirect_dependent.len();*/
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
    // let mut count2 = 0;
    /*for item in indirect_dependent {
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
    }*/

    let res_deps = DependentInfo {
        direct_count: getdirect_count,
        indirect_count: 0,
        data: deps,
    };
    HttpResponse::Ok().json(res_deps)
}

/// 获取被依赖关系
#[utoipa::path(
    get,
    path = "/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependents",
    params(
        ("nsfront" = String, Path, description = "命名空间前缀"),
        ("nsbehind" = String, Path, description = "命名空间后缀"), 
        ("cratename" = String, Path, description = "crate 名称"),
        ("version" = String, Path, description = "版本号")
    ),
    responses(
        (status = 200, description = "成功获取被依赖关系", body = DependentInfo),
        (status = 404, description = "未找到被依赖关系")
    ),
    tag = "dependents"
)]
pub async fn dependent_cache(
    name: String,
    version: String,
    nsfront: String,
    nsbehind: String,
) -> impl Responder {
    let handler = get_tugraph_api_handler().await;
    let namespace = nsfront.clone() + "/" + &nsbehind.clone();
    let nameversion = name.clone() + "/" + &version.clone();
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    let res = dbhandler
        .get_dependent_from_pg(
            nsfront.clone(),
            nsbehind.clone(),
            name.clone(),
            version.clone(),
        )
        .await
        .unwrap();
    if res.is_empty() {
        let direct_nodes = handler
            .reader
            .new_get_direct_dependent_nodes(&namespace, &nameversion)
            .await
            .unwrap();
        let getdirect_count = direct_nodes.len();
        let all_dependent_nodes = handler
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
        dbhandler
            .insert_dependent_into_pg(
                nsfront.clone(),
                nsbehind.clone(),
                name.clone(),
                version.clone(),
                res_deps.clone(),
            )
            .await
            .unwrap();
        HttpResponse::Ok().json(res_deps)
    } else {
        HttpResponse::Ok().json(res[0].clone())
    }
}
/// 查询 crates
#[utoipa::path(
    post,
    path = "/api/search",
    request_body = Query,
    responses(
        (status = 200, description = "查询成功", body = QueryCratesInfo),
        (status = 400, description = "无效的查询参数")
    ),
    tag = "search"
)]
pub async fn query_crates(q: Query) -> impl Responder {
    let _handler = get_tugraph_api_handler().await;
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
    let db_connection_config = db_connection_config_from_env();
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
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
    tracing::trace!("search need time:{:?}", start_time2.elapsed());
    //
    let mut seen = HashSet::new();
    let uniq_res: Vec<RecommendCrate> = res
        .into_iter()
        .filter(|x| seen.insert((x.name.clone(), x.namespace.clone())))
        .collect();
    tracing::trace!("total programs: {}", uniq_res.len());
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
//post of upload
pub async fn upload_crate(mut payload: Multipart) -> impl Responder {
    tracing::info!("enter upload crate");
    use futures_util::StreamExt as _;
    let mut upload_time: Option<String> = None;
    let mut user_email: Option<String> = None;
    let mut github_link: Option<String> = None;
    let mut file_name: Option<String> = None;
    while let Some(Ok(mut field)) = payload.next().await {
        tracing::info!("enter while");
        if let Some(content_disposition) = field.content_disposition() {
            tracing::info!("enter first if");
            if let Some(name) = content_disposition.get_name() {
                tracing::info!("enter second if");
                match name {
                    "file" => {
                        tracing::info!("enter match file");
                        let filename = if let Some(file_name) = content_disposition.get_filename() {
                            file_name.to_string()
                        } else {
                            "default.zip".to_string()
                        };
                        tracing::info!("filename:{}", filename.clone());
                        let sanitized_filename = sanitize(filename.clone());
                        file_name = Some(filename.clone());
                        tracing::info!("file_name:{:?}", file_name.clone());
                        if sanitized_filename.ends_with(".zip") {
                            tracing::info!("enter file zip");
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
                                        format!(
                                            "target/www/uploads/{}/{}",
                                            filename,
                                            path.display()
                                        )
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
                                    let mut outfile =
                                        tokio::fs::File::create(&outpath).await.unwrap();
                                    while let Ok(bytes_read) = file.read(&mut buffer) {
                                        if bytes_read == 0 {
                                            break;
                                        }
                                        outfile.write_all(&buffer[..bytes_read]).await.unwrap();
                                    }
                                }
                            }
                            tracing::info!("finish match file");
                            //send message
                            /*let send_url = format!("target/www/uploads/{}", filename);
                            let sent_payload = repo_sync_model::Model {
                                id: 0,
                                crate_name: filename,
                                github_url: None,
                                mega_url: send_url,
                                crate_type: CrateType::Lib,
                                status: model::repo_sync_model::RepoSyncStatus::Syncing,
                                err_message: None,
                            };
                            let kafka_user_import_topic =
                                env::var("KAFKA_USER_IMPORT_TOPIC").unwrap();
                            let import_driver = ImportDriver::new(false).await;
                            let _ = import_driver
                                .user_import_handler
                                .send_message(
                                    &kafka_user_import_topic,
                                    "",
                                    &serde_json::to_string(&sent_payload).unwrap(),
                                )
                                .await;
                            break;*/
                        } else {
                            tracing::info!("enter else");
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
                    }
                    "githubLink" => {
                        let mut url_data = Vec::new();

                        // 读取字段的内容
                        while let Some(chunk) = field.next().await {
                            let data = chunk.unwrap();
                            url_data.extend_from_slice(&data);
                        }
                        github_link = Some(String::from_utf8(url_data).unwrap_or_default());
                    }
                    "uploadTime" => {
                        tracing::info!("enter match uploadtime");
                        let mut time_data = Vec::new();

                        while let Some(chunk) = field.next().await {
                            let data = chunk.unwrap();
                            time_data.extend_from_slice(&data);
                        }
                        upload_time = Some(String::from_utf8(time_data).unwrap_or_default());
                        tracing::info!("uploadtime:{:?}", upload_time);
                    }
                    "user_email" => {
                        tracing::info!("enter match user_email");
                        let mut email_data = Vec::new();

                        while let Some(chunk) = field.next().await {
                            let data = chunk.unwrap();
                            email_data.extend_from_slice(&data);
                        }
                        user_email = Some(String::from_utf8(email_data).unwrap_or_default());
                        tracing::info!("user_email:{:?}", user_email);
                    }
                    _ => {
                        tracing::info!("enter match nothing");
                    }
                }
            }
        }
    }
    if let Some(filename) = file_name {
        tracing::info!("enter 1/2 if let");
        let db_connection_config = db_connection_config_from_env();
        #[allow(unused_variables)]
        let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
            .await
            .unwrap();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let dbhandler = DBHandler { client };
        if let Some(uploadtime) = upload_time.clone() {
            tracing::info!("enter upload time:{}", uploadtime.clone());
            if let Some(useremail) = user_email.clone() {
                tracing::info!("enter user email:{}", useremail.clone());
                dbhandler
                    .client
                    .execute(
                        "INSERT INTO uploadedcrate(email,filename,uploadtime) VALUES ($1, $2,$3);",
                        &[&useremail.clone(), &filename.clone(), &uploadtime.clone()],
                    )
                    .await
                    .unwrap();
            }
        }
    };
    if let Some(githublink) = github_link {
        tracing::info!("enter 2/2 if let");
        let db_connection_config = db_connection_config_from_env();
        #[allow(unused_variables)]
        let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
            .await
            .unwrap();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        let dbhandler = DBHandler { client };
        if let Some(uploadtime) = upload_time.clone() {
            if let Some(useremail) = user_email.clone() {
                dbhandler
                    .client
                    .execute(
                        "INSERT INTO uploadedurl(email,githuburl,uploadtime) VALUES ($1, $2,$3);",
                        &[&useremail.clone(), &githublink.clone(), &uploadtime.clone()],
                    )
                    .await
                    .unwrap();
            }
        }
    }
    HttpResponse::Ok().json(())
}
//post of log in
pub async fn submituserinfo(info: Userinfo) -> impl Responder {
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    tracing::info!("enter submituserinfo and set db client");
    #[allow(clippy::let_unit_value)]
    let _ = dbhandler
        .insert_userinfo_into_pg(info.clone())
        .await
        .unwrap();
    HttpResponse::Ok().json(())
}
pub async fn query_upload_crate(email: String) -> impl Responder {
    let db_connection_config = db_connection_config_from_env();
    #[allow(unused_variables)]
    let (client, connection) = tokio_postgres::connect(&db_connection_config, NoTls)
        .await
        .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let dbhandler = DBHandler { client };
    let mut real_res = vec![];
    let res = dbhandler
        .query_uploaded_crates_from_pg(email.clone())
        .await
        .unwrap();
    for row in res {
        real_res.push(row);
    }
    let res2 = dbhandler
        .query_uploaded_url_from_pg(email.clone())
        .await
        .unwrap();
    for row in res2 {
        real_res.push(row);
    }
    HttpResponse::Ok().json(real_res)
}
