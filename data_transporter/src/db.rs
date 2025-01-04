use std::{cmp::Ordering, collections::HashSet};

use crate::route::{Crateinfo, DependencyCount, DependentCount, RustSec};
use model::tugraph_model::{Program, UProgram, UVersion};
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio_postgres::{Error, NoTls};
pub struct DBHandler {
    pub client: tokio_postgres::Client,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CveInfo {
    cve_id: String,
    url: String,
    description: String,
    crate_name: String,
    start_version: String,
    end_version: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Allcve {
    cves: Vec<CveInfo>,
}
impl DBHandler {
    pub async fn connect() -> Result<Self, Error> {
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=postgres",
            NoTls,
        )
        .await?;

        // Spawn the connection on a separate task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        // 创建 cratespro 数据库
        client
            .execute("CREATE DATABASE cratespro", &[])
            .await
            .or_else(|err| {
                if let Some(db_err) = err.as_db_error() {
                    if db_err.code() == &tokio_postgres::error::SqlState::DUPLICATE_DATABASE {
                        return Ok(0);
                    }
                }
                Err(err)
            })?;

        // 重新连接到 cratespro 数据库
        let (client, connection) = tokio_postgres::connect(
            "host=172.17.0.1 port=30432 user=mega password=mega dbname=cratespro",
            NoTls,
        )
        .await?;

        // Spawn the connection on a separate task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(DBHandler { client })
    }

    pub async fn clear_database(&self) -> Result<(), Error> {
        self.client
            .batch_execute(
                "
                DO $$
                BEGIN
                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'programs') THEN
                        DROP TABLE programs CASCADE;
                    END IF;


                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'program_versions') THEN
                        DROP TABLE program_versions CASCADE;
                    END IF;

                    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'program_dependencies') THEN
                        DROP TABLE program_dependencies CASCADE;
                    END IF;
                    

                END $$;
                ",
            )
            .await
    }

    pub async fn create_tables(&self) -> Result<(), Error> {
        let create_programs_table = "
            CREATE TABLE IF NOT EXISTS programs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                namespace TEXT,
                max_version TEXT,
                github_url TEXT,
                mega_url TEXT,
                doc_url TEXT,
                program_type TEXT NOT NULL,
                downloads BIGINT,
                cratesio TEXT
            );
        ";

        let create_program_versions_table = "
            CREATE TABLE IF NOT EXISTS program_versions (
                name_and_version TEXT PRIMARY KEY,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                documentation TEXT,
                version_type TEXT NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            );
        ";

        let create_program_dependencies_table = "
            CREATE TABLE IF NOT EXISTS program_dependencies (
                name_and_version TEXT NOT NULL,
                dependency_name TEXT NOT NULL,
                dependency_version TEXT NOT NULL,
                PRIMARY KEY (name_and_version, dependency_name, dependency_version)
            );
        ";

        // 执行创建表的 SQL 语句
        let result = self
            .client
            .batch_execute(&format!(
                "{}{}{}",
                create_programs_table,
                create_program_versions_table,
                create_program_dependencies_table
            ))
            .await;

        match result {
            Ok(_) => {
                tracing::info!("Tables created successfully.");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Error creating tables: {:?}", e);
                Err(e)
            }
        }
    }
    pub async fn insert_program_data(
        &self,
        program: Program,
        uprogram: UProgram,
        versions: Vec<crate::VersionInfo>,
    ) -> Result<(), Error> {
        let (program_type, downloads, cratesio) = match &uprogram {
            UProgram::Library(lib) => ("Library", Some(lib.downloads), lib.cratesio.clone()),
            UProgram::Application(_) => ("Application", None, None),
        };

        self.client
            .execute(
                "
            INSERT INTO programs (
                id, name, description, namespace, 
                max_version, github_url, mega_url, doc_url,
                program_type, downloads, cratesio
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ",
                &[
                    &program.id,
                    &program.name,
                    &program.description.unwrap_or_default(),
                    &program.namespace.unwrap_or_default(),
                    &program.max_version.unwrap_or_default(),
                    &program.github_url.unwrap_or_default(),
                    &program.mega_url.unwrap_or_default(),
                    &program.doc_url.unwrap_or_default(),
                    &program_type,
                    &downloads.unwrap_or_default(),
                    &cratesio.unwrap_or_default(),
                ],
            )
            .await
            .map_err(|e| {
                eprintln!("Error inserting program: {:?}", e);
                e
            })
            .unwrap();

        tracing::info!("finish to insert program.");

        // 插入 UVersion 数据
        for version in versions {
            let name_and_version = version.version_base.get_name_and_version();

            match version.version_base {
                UVersion::LibraryVersion(lib_ver) => {
                    self.client
                        .execute(
                            "
                        INSERT INTO program_versions (
                            name_and_version, id, name, version, 
                            documentation, version_type, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
                        ",
                            &[
                                &lib_ver.name_and_version,
                                &lib_ver.id,
                                &lib_ver.name,
                                &lib_ver.version,
                                &Some(lib_ver.documentation),
                                &"LibraryVersion",
                            ],
                        )
                        .await
                        .unwrap();
                }
                UVersion::ApplicationVersion(app_ver) => {
                    self.client
                        .execute(
                            "
                        INSERT INTO program_versions (
                            name_and_version, id, name, version, 
                            documentation, version_type, created_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
                        ",
                            &[
                                &app_ver.name_and_version,
                                &app_ver.id,
                                &app_ver.name,
                                &app_ver.version,
                                &None::<String>, // ApplicationVersion 没有 documentation 字段
                                &"ApplicationVersion",
                            ],
                        )
                        .await
                        .unwrap();
                }
            }

            // 插入该版本的所有依赖项
            for dep in version.dependencies {
                self.client
                    .execute(
                        "
                        INSERT INTO program_dependencies (
                            name_and_version, dependency_name, dependency_version
                        ) VALUES ($1, $2, $3)
                        ",
                        &[&name_and_version, &dep.name, &dep.version],
                    )
                    .await?;
            }
        }
        tracing::info!("Finish to insert all versions.");

        Ok(())
    }
    pub async fn get_all_cvelist(&self) -> Result<Allcve, Error> {
        //let getcve = "SELECT cve_id, name, start_version, end_version FROM cves;";

        let raws = self
            .client
            .query(
                "SELECT cve_id, name, start_version, end_version,description FROM cves;",
                &[],
            )
            .await?;
        let mut getcves = vec![];
        for raw in raws {
            let front = "https://www.cve.org/CVERecord?id=";
            let cve_id: String = raw.get(0);
            let cve_url = front.to_string() + &cve_id;
            let cve_info = CveInfo {
                cve_id: raw.get(0),
                url: cve_url,
                description: raw.get(4),
                crate_name: raw.get(1),
                start_version: raw.get(2),
                end_version: raw.get(3),
            };
            getcves.push(cve_info);
        }
        let res = Allcve { cves: getcves };

        Ok(res)
    }
    pub async fn match_version(&self, patched: String, version: String) -> Result<bool, Error> {
        let mut matched = false;
        let mut part_petched = vec![];
        let parts: Vec<&str> = patched.split('|').collect();
        for part in parts {
            part_petched.push(part);
        }
        for np in part_petched {
            let oneline_patched = np.to_string();
            if oneline_patched.contains(",") {
                //闭区间
                let mut two_versions = vec![];
                let newparts: Vec<&str> = oneline_patched.split(',').collect();
                for part in newparts {
                    let one_version = part.to_string();
                    let res_one_version = one_version.trim();
                    two_versions.push(res_one_version.to_string());
                }
                let mut left = "".to_string();
                let mut right = "".to_string();
                if two_versions.len() == 2 {
                    if two_versions[0].clone().starts_with(">")
                        || two_versions[0].clone().starts_with(">=")
                    {
                        left = two_versions[0].clone();
                        right = two_versions[1].clone();
                    } else if two_versions[0].clone().starts_with("<")
                        || two_versions[0].clone().starts_with("<=")
                    {
                        left = two_versions[1].clone();
                        right = two_versions[0].clone();
                    }
                }
                if (left.starts_with(">") && !left.starts_with(">="))
                    && (right.starts_with("<") && !right.starts_with("<="))
                {
                    //> <
                    let mut versions = vec![];
                    let tmp_left = &left[1..];
                    let left_version = tmp_left.to_string();
                    let tmp_right = &right[1..];
                    let right_version = tmp_right.to_string();
                    versions.push(version.clone());
                    versions.push(left_version.clone());
                    versions.push(right_version.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if version.clone() == versions[1].clone()
                        && (versions[0].clone() != version.clone()
                            && versions[2].clone() != version.clone())
                    {
                        matched = true;
                    }
                } else if (left.starts_with(">") && !left.starts_with(">="))
                    && right.starts_with("<=")
                {
                    //> <=
                    let mut versions = vec![];
                    let tmp_left = &left[1..];
                    let left_version = tmp_left.to_string();
                    let tmp_right = &right[2..];
                    let right_version = tmp_right.to_string();
                    versions.push(version.clone());
                    versions.push(left_version.clone());
                    versions.push(right_version.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if version.clone() == versions[1].clone()
                        && (versions[2].clone() != version.clone())
                    {
                        matched = true;
                    }
                } else if left.starts_with(">=")
                    && (right.starts_with("<") && !right.starts_with("<="))
                {
                    //>= <
                    //println!("1");
                    let mut versions = vec![];
                    let tmp_left = &left[2..];
                    let left_version = tmp_left.to_string();
                    let tmp_right = &right[1..];
                    let right_version = tmp_right.to_string();
                    versions.push(version.clone());
                    versions.push(left_version.clone());
                    versions.push(right_version.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    //println!("{} {} {}",versions[0].clone(),versions[1].clone(),versions[2].clone());
                    if version.clone() == versions[1].clone()
                        && (versions[0].clone() != version.clone())
                    {
                        matched = true;
                    }
                } else if left.starts_with(">=") && right.starts_with("<=") {
                    //>= <=
                    let mut versions = vec![];
                    let tmp_left = &left[2..];
                    let left_version = tmp_left.to_string();
                    let tmp_right = &right[2..];
                    let right_version = tmp_right.to_string();
                    versions.push(version.clone());
                    versions.push(left_version.clone());
                    versions.push(right_version.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if version.clone() == versions[1].clone() {
                        matched = true;
                    }
                }
            } else if oneline_patched.contains("^") {
                //具体版本
                if let Some(trimmed) = oneline_patched.strip_prefix("^") {
                    let res = trimmed.to_string();
                    if version == res {
                        matched = true;
                    }
                }
            } else {
                //单侧区间
                if oneline_patched.starts_with(">") && !oneline_patched.starts_with(">=") {
                    let mut versions = vec![];
                    let trimmed = &oneline_patched[1..];
                    let res = trimmed.to_string();
                    versions.push(version.clone());
                    versions.push(res.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if versions[0].clone() == version.clone() && res.clone() != version.clone() {
                        //println!("1");
                        matched = true;
                    }
                } else if let Some(trimmed) = oneline_patched.strip_prefix(">=") {
                    let mut versions = vec![];
                    let res = trimmed.to_string();
                    versions.push(version.clone());
                    versions.push(res.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    //println!("{} {}",versions[0].clone(),versions[1].clone());
                    if versions[0].clone() == version.clone() {
                        //println!("1");
                        matched = true;
                    }
                } else if oneline_patched.starts_with("<") && !oneline_patched.starts_with("<=") {
                    //println!("1");
                    let mut versions = vec![];
                    let trimmed = &oneline_patched[1..];
                    let res = trimmed.to_string();
                    versions.push(version.clone());
                    versions.push(res.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    //println!("|{}| |{}|",versions[0].clone(),versions[1].clone());
                    if versions[1].clone() == version.clone() && res.clone() != version.clone() {
                        matched = true;
                    }
                } else if let Some(trimmed) = oneline_patched.strip_prefix("<=") {
                    let mut versions = vec![];
                    let res = trimmed.to_string();
                    versions.push(version.clone());
                    versions.push(res.clone());
                    versions.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);
                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if versions[1].clone() == version.clone() {
                        matched = true;
                    }
                }
            }
        }
        Ok(matched)
    }
    pub async fn get_direct_rustsec(
        &self,
        cname: &str,
        version: &str,
    ) -> Result<Vec<RustSec>, Error> {
        let rows = self
            .client
            .query("SELECT * FROM rustsecs;", &[])
            .await
            .unwrap();
        let mut get_direct_rust_sec = vec![];
        for row in rows {
            let t_aliases: String = row.get("aliases");
            let parts: Vec<&str> = t_aliases.split(';').collect();
            let mut real_aliases = vec![];
            for part in parts {
                real_aliases.push(part.to_string());
            }
            let rs = RustSec {
                id: row.get("id"),
                cratename: row.get("cratename"),
                patched: row.get("patched"),
                aliases: real_aliases.clone(),
                small_desc: row.get("small_desc"),
            };
            get_direct_rust_sec.push(rs.clone());
        }
        let mut getres = vec![];
        for rc in get_direct_rust_sec {
            if rc.cratename.clone() == *cname {
                let matched = self
                    .match_version(rc.clone().patched, version.to_string())
                    .await
                    .unwrap();
                if !matched {
                    getres.push(rc.clone());
                }
            }
        }
        Ok(getres)
    }
    pub async fn get_dependency_rustsec(
        &self,
        nameversion: HashSet<String>,
    ) -> Result<Vec<RustSec>, Error> {
        let rows = self
            .client
            .query("SELECT * FROM rustsecs;", &[])
            .await
            .unwrap();
        let mut get_all_rust_sec = vec![];
        for row in rows {
            let t_aliases: String = row.get("aliases");
            let parts: Vec<&str> = t_aliases.split(';').collect();
            let mut real_aliases = vec![];
            for part in parts {
                real_aliases.push(part.to_string());
            }
            let rs = RustSec {
                id: row.get("id"),
                cratename: row.get("cratename"),
                patched: row.get("patched"),
                aliases: real_aliases.clone(),
                small_desc: row.get("small_desc"),
            };
            get_all_rust_sec.push(rs.clone());
        }
        let mut getres = vec![];
        for nv in nameversion {
            let parts: Vec<&str> = nv.split('/').collect();
            let cname = parts[0].to_string();
            let version = parts[1].to_string();
            for rc in get_all_rust_sec.clone() {
                if rc.cratename.clone() == cname {
                    let matched = self
                        .match_version(rc.clone().patched, version.to_string())
                        .await
                        .unwrap();
                    if !matched {
                        getres.push(rc.clone());
                    }
                }
            }
        }
        let unique: Vec<RustSec> = getres
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Ok(unique)
    }
    #[allow(dead_code)]
    pub async fn get_direct_cve_by_cratenameandversion(
        &self,
        cratename: &str,
        version: &str,
    ) -> Result<Vec<String>, Error> {
        /*let rows = self
            .client
            .query(
                "SELECT cve_id FROM cves WHERE name = $1;",
                &[&cratename.to_string()],
            )
            .await
            .unwrap();
        let mut cves = vec![];
        for row in rows {
            let cve_id: String = row.get(0);
            cves.push(cve_id);
        }
        Ok(cves)*/
        let rows = self.client.query("SELECT * FROM cves;", &[]).await.unwrap();
        let mut getallcves = vec![];
        for row in rows {
            let cveinfo = CveInfo {
                cve_id: row.get("cve_id"),
                url: "".to_string(),
                description: row.get("description"),
                crate_name: row.get("name"),
                start_version: row.get("start_version"),
                end_version: row.get("end_version"),
            };
            getallcves.push(cveinfo);
        }
        let mut getres = vec![];
        for cveinfo in getallcves {
            let mut version3 = vec![];
            if cveinfo.crate_name.clone() == *cratename {
                version3.push(cveinfo.start_version.clone());
                version3.push(cveinfo.end_version.clone());
                version3.push(version.to_string());
                version3.sort_by(|a, b| {
                    let version_a = Version::parse(a);
                    let version_b = Version::parse(b);

                    match (version_a, version_b) {
                        (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                        (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => Ordering::Equal,
                    }
                });
                if version3[1].clone() == *version {
                    getres.push(cveinfo.cve_id.clone());
                }
            }
        }

        Ok(getres)
    }
    #[allow(dead_code)]
    pub async fn get_dependency_cve_by_cratenameandversion(
        &self,
        nameversion: HashSet<String>,
    ) -> Result<Vec<String>, Error> {
        let rows = self.client.query("SELECT * FROM cves;", &[]).await.unwrap();
        let mut getallcves = vec![];
        for row in rows {
            let cveinfo = CveInfo {
                cve_id: row.get("cve_id"),
                url: "".to_string(),
                description: row.get("description"),
                crate_name: row.get("name"),
                start_version: row.get("start_version"),
                end_version: row.get("end_version"),
            };
            getallcves.push(cveinfo);
        }
        let mut getres = vec![];
        for nv in nameversion {
            let parts: Vec<&str> = nv.split('/').collect();
            let cratename = parts[0].to_string();
            let crateversion = parts[1].to_string();
            for cveinfo in getallcves.clone() {
                let mut version3 = vec![];
                if cveinfo.crate_name.clone() == *cratename {
                    version3.push(cveinfo.start_version.clone());
                    version3.push(cveinfo.end_version.clone());
                    version3.push(crateversion.to_string());
                    version3.sort_by(|a, b| {
                        let version_a = Version::parse(a);
                        let version_b = Version::parse(b);

                        match (version_a, version_b) {
                            (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // 从高到低排序
                            (Ok(_), Err(_)) => Ordering::Less,   // 无法解析的版本号认为更小
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => Ordering::Equal,
                        }
                    });
                    if version3[1].clone() == *crateversion {
                        getres.push(cveinfo.cve_id.clone());
                    }
                }
            }
        }
        let unique: Vec<String> = getres
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        Ok(unique)
    }
    pub async fn get_license_by_name(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Vec<String>, Error> {
        let rows = self
            .client
            .query(
                "SELECT license FROM license WHERE program_namespace = $1 and program_name = $2;",
                &[&namespace.to_string(), &name.to_string()],
            )
            .await
            .unwrap();
        let mut licenses = vec![];
        for row in rows {
            let new_license: String = row.get(0);
            licenses.push(new_license);
        }
        Ok(licenses)
    }
    pub async fn query_crates_info_from_pg(
        &self,
        id: &str,
        name: String,
    ) -> Result<Vec<Crateinfo>, Box<dyn std::error::Error>> {
        let rows = self
            .client
            .query(
                "SELECT * FROM crates_info WHERE id = $1;",
                &[&id.to_string()],
            )
            .await
            .unwrap();
        let mut cf = vec![];
        for row in rows {
            let desc: String = row.get("description");
            let dcyct: i32 = row.get("direct_dependency");
            let indcyct: i32 = row.get("indirect_dependency");
            let dtct: i32 = row.get("direct_dependent");
            let indtct: i32 = row.get("indirect_dependent");
            let cs: String = row.get("cves");
            let vs: String = row.get("versions");
            let lcs: String = row.get("license");
            let gu: String = row.get("github_url");
            let du: String = row.get("doc_url");
            let dep_cs: String = row.get("dep_cves");
            let mut getcves = vec![];
            let everypartscs: Vec<&str> = cs.split('|').collect();
            for part in everypartscs {
                let new_part = part.to_string();
                let parts2: Vec<&str> = new_part.split('/').collect();
                if parts2.len() == 2 {
                    let empty_vec: Vec<String> = Vec::new();
                    let onecve = RustSec {
                        id: parts2[0].to_string(),
                        cratename: "".to_string(),
                        patched: "".to_string(),
                        aliases: empty_vec,
                        small_desc: parts2[1].to_string(),
                    };
                    getcves.push(onecve);
                } else if parts2.len() == 3 {
                    let part2clone = parts2[2].to_string();
                    let tp: Vec<&str> = part2clone.split(';').collect();
                    let mut real_aliases = vec![];
                    for part in tp {
                        real_aliases.push(part.to_string());
                    }
                    let onecve = RustSec {
                        id: parts2[0].to_string(),
                        cratename: "".to_string(),
                        patched: "".to_string(),
                        aliases: real_aliases,
                        small_desc: parts2[1].to_string(),
                    };
                    getcves.push(onecve);
                }
            }
            let mut getdepcs = vec![];
            let everypartsdepcs: Vec<&str> = dep_cs.split('|').collect();
            for part in everypartsdepcs {
                let new_part = part.to_string();
                let parts2: Vec<&str> = new_part.split('/').collect();
                if parts2.len() == 2 {
                    let empty_vec: Vec<String> = Vec::new();
                    let onecve = RustSec {
                        id: parts2[0].to_string(),
                        cratename: "".to_string(),
                        patched: "".to_string(),
                        aliases: empty_vec,
                        small_desc: parts2[1].to_string(),
                    };
                    getdepcs.push(onecve);
                } else if parts2.len() == 3 {
                    let part2clone = parts2[2].to_string();
                    let tp: Vec<&str> = part2clone.split(';').collect();
                    let mut real_aliases = vec![];
                    for part in tp {
                        real_aliases.push(part.to_string());
                    }
                    let onecve = RustSec {
                        id: parts2[0].to_string(),
                        cratename: "".to_string(),
                        patched: "".to_string(),
                        aliases: real_aliases,
                        small_desc: parts2[1].to_string(),
                    };
                    getdepcs.push(onecve);
                }
            }
            let mut getversions = vec![];
            let partsvs: Vec<&str> = vs.split('/').collect();
            for part in partsvs {
                getversions.push(part.to_string());
            }
            let res_crates_info = Crateinfo {
                crate_name: name.clone(),
                description: desc.clone(),
                dependencies: DependencyCount {
                    direct: dcyct as usize,
                    indirect: indcyct as usize,
                },
                dependents: DependentCount {
                    direct: dtct as usize,
                    indirect: indtct as usize,
                },
                cves: getcves,
                license: lcs.clone(),
                github_url: gu.clone(),
                doc_url: du.clone(),
                versions: getversions,
                dep_cves: getdepcs,
            };
            cf.push(res_crates_info);
        }
        Ok(cf)
    }
    pub async fn insert_crates_info_into_pg(
        &self,
        crateinfo: Crateinfo,
        namespace: String,
        name: String,
        version: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let id = namespace.clone() + "/" + &name + "/" + &version;
        let dcyct = crateinfo.dependencies.direct as i32;
        let indcyct = crateinfo.dependencies.indirect as i32;
        let dtct = crateinfo.dependents.direct as i32;
        let indtct = crateinfo.dependents.indirect as i32;
        let vs = crateinfo.versions.clone().join("/");
        let mut every_cs = vec![];
        for rs in crateinfo.clone().cves {
            let t_id = rs.clone().id;
            let t_small_desc = rs.clone().small_desc;
            let t_aliases = rs.clone().aliases.join(";");
            let tmp_strings = [t_id, t_small_desc, t_aliases];
            let result: String = tmp_strings
                .iter()
                .filter(|&s| !s.is_empty())
                .cloned() // 复制引用的字符串
                .collect::<Vec<String>>()
                .join("/");
            every_cs.push(result);
        }
        let cs = every_cs.clone().join("|");
        let mut every_dep_cs = vec![];
        for rs in crateinfo.clone().dep_cves {
            let t_id = rs.clone().id;
            let t_small_desc = rs.clone().small_desc;
            let t_aliases = rs.clone().aliases.join(";");
            let tmp_strings = [t_id, t_small_desc, t_aliases];
            let result: String = tmp_strings
                .iter()
                .filter(|&s| !s.is_empty())
                .cloned() // 复制引用的字符串
                .collect::<Vec<String>>()
                .join("/");
            every_dep_cs.push(result);
        }
        let depcs = every_dep_cs.clone().join("|");
        self.client
            .execute(
                "
                        INSERT INTO crates_info (
                            id,description,direct_dependency,indirect_dependency,
                            direct_dependent,indirect_dependent,cves,dep_cves,versions,
                            license,github_url,doc_url
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7,$8,$9,$10,$11,$12);
                        ",
                &[
                    &id,
                    &crateinfo.description,
                    &dcyct,
                    &indcyct,
                    &dtct,
                    &indtct,
                    &cs,
                    &depcs,
                    &vs,
                    &crateinfo.license,
                    &crateinfo.github_url,
                    &crateinfo.doc_url,
                ],
            )
            .await
            .unwrap();
        Ok(())
    }
}
