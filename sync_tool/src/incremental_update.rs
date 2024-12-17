use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    env,
    fs::{self, File},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{exit, Command},
    str::FromStr,
    time::Duration,
};

use chrono::Utc;
use entity::{db_enums::RepoSyncStatus, repo_sync_status};
use flate2::bufread::GzDecoder;
use git2::{Repository, Signature};
use kafka_model::message_model;
use rdkafka::producer::FutureProducer;
use regex::Regex;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, Unchanged};
use tar::Archive;
use tokio::time::sleep;
use url::Url;
use walkdir::WalkDir;

use crate::{
    kafka::{self},
    util,
};

// 定义一个结构体来存储从日志文件中解析出的信息
#[derive(Debug)]
struct CrateInfo {
    path: PathBuf,
    full_path: PathBuf,
    version: String,
}

pub async fn incremental_update() {
    loop {
        //一直执行 一次更新之后再300s又重来
        let task_duration = Duration::from_secs(2);
        sleep(task_duration).await;
        let conn = util::db_connection().await;
        let producer = kafka::get_producer();

        // 记录上次日志的末尾
        let log_file_path = Path::new("/home/rust/freighter/log/crates.log");
        //let log_file_path = Path::new("/home/rust/workspace/tools/src/crates.log");
        println!("Trying to open: {}", log_file_path.display());
        let temp_file = File::open(log_file_path).expect("Failed to open file");
        let temp_reader = BufReader::new(temp_file);
        let last_location = temp_reader.lines().count();
        let last_location_u64: u64 = last_location as u64;

        //freighter 增量更新
        Command::new("/home/rust/.cargo/bin/freighter-registry")
            .arg("crates")
            .arg("download")
            .output()
            .expect("Failed to execute `freighter crates download`");

        println!("freighter-done!");
        // 从日志中读出增量信息
        let download_txt_dir = Path::new("/home/rust/freighter/log/");
        let crate_names = read_latest_crate_list(download_txt_dir)
            .expect("Failed to get download.txt crates name");
        let crates_info = parse_log_file(log_file_path, last_location_u64, &crate_names)
            .expect("Failed to parse log file");

        println!("log_read-done!");
        for crate_info in &crates_info {
            println!("re: {:?}", crate_info);

            let crate_path = &crate_info.path;
            let crate_name = crate_path.file_name().unwrap().to_str().unwrap();
            let crate_full_path = crate_info.full_path.as_path();
            let crate_entry = crate_path.as_path();
            let repo_path = &crate_path.join(crate_name);

            let record = crate::get_record(&conn, crate_name).await;
            if record.status == Unchanged(RepoSyncStatus::Succeed) {
                continue;
            }

            if repo_path.exists() {
                println!("repo_path: {}", repo_path.display());
                match fs::remove_dir_all(repo_path) {
                    Ok(()) => (),
                    Err(e) => println!("Failed to remove directory: {}", e),
                }
            }

            let repo = open_or_make_repo(repo_path);

            decompress_crate_file(crate_full_path, crate_entry).unwrap_or_else(|e| {
                eprintln!("{}", e);
            });

            let uncompress_path = remove_extension(crate_full_path);
            if fs::read_dir(&uncompress_path).is_err() {
                continue;
            }
            empty_folder(repo.workdir().unwrap()).unwrap();
            copy_all_files(&uncompress_path, repo.workdir().unwrap()).unwrap();

            //提取版本号并提交
            let version = &crate_info.version;
            add_and_commit(&repo, version, repo_path);

            match fs::remove_dir_all(uncompress_path) {
                Ok(()) => (),
                Err(e) => println!("Failed to remove uncompress_path: {}", e),
            }
            //fs::remove_dir_all(uncompress_path).unwrap();

            if repo_path.exists() {
                //push 到mega 进行存储（其中推送到kafka
                push_to_remote(&conn, crate_name, repo_path, record, &producer).await;
            } else {
                eprintln!("empty crates directory:{:?}", crate_entry)
            }
        }

        sleep(Duration::from_secs(300)).await;
    }

    fn open_or_make_repo(repo_path: &Path) -> Repository {
        match Repository::open(repo_path) {
            Ok(repo) => repo,
            Err(_) => {
                println!("Creating a new repository...");
                // Create a new repository
                match Repository::init(repo_path) {
                    Ok(repo) => {
                        println!(
                            "Successfully created a new repository at: {}",
                            repo_path.display()
                        );
                        repo
                    }
                    Err(e) => {
                        panic!("Failed to create a new repository: {}", e);
                    }
                }
            }
        }
    }

    fn add_and_commit(repo: &Repository, version: &str, repo_path: &Path) {
        if let Err(err) = env::set_current_dir(repo_path) {
            eprintln!("Error changing directory: {}", err);
            exit(1);
        }
        // Add all changes in the working directory to the index
        Command::new("git").arg("add").arg("./").output().unwrap();

        // Get the repository index
        let mut index = repo.index().unwrap();

        index.write().unwrap();

        // Create a tree from the index
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        // Get the current HEAD commit (if any)
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit().unwrap()),
            Err(_) => None,
        };

        // Create a signature
        let sig = Signature::now("Mega", "admin@mega.com").unwrap();

        // Create a new commit
        let commit_id = if let Some(parent) = parent_commit {
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit Version: {}", version),
                &tree,
                &[&parent],
            )
            .unwrap()
        } else {
            // Initial commit (no parents)
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit Version: {}", version),
                &tree,
                &[],
            )
            .unwrap()
        };

        // Create the tag
        //repo.tag_lightweight(version, &repo.find_object(commit_id, None).unwrap(), false).unwrap();
        match repo.tag_lightweight(version, &repo.find_object(commit_id, None).unwrap(), false) {
            Ok(_) => (), // 成功时什么也不做
            Err(e) => match e.code() {
                git2::ErrorCode::Exists => println!("Tag '{}' already exists.", version),
                _ => println!("Failed to create tag: {}", e.message()),
            },
        }
    }

    fn copy_all_files(src: &Path, dst: &Path) -> io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }

        for entry in fs::read_dir(src).unwrap() {
            let entry = entry?;
            let path = entry.path();
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };
            let dest_path = dst.join(file_name);

            if path.is_dir() {
                if !path.ends_with(".git") {
                    copy_all_files(&path, &dest_path).unwrap();
                }
            } else {
                fs::copy(&path, &dest_path).unwrap();
            }
        }
        Ok(())
    }

    fn empty_folder(dir: &Path) -> io::Result<()> {
        for entry in WalkDir::new(dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                fs::remove_file(path)?;
            } else if path.is_dir() && path.file_name().unwrap() != ".git" {
                fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

    async fn push_to_remote(
        conn: &DatabaseConnection,
        crate_name: &str,
        repo_path: &Path,
        mut record: repo_sync_status::ActiveModel,
        producer: &FutureProducer,
    ) {
        if let Err(err) = env::set_current_dir(repo_path) {
            eprintln!("Error changing directory: {}", err);
            exit(1);
        }

        //let mut url = Url::from_str("http://localhost:8000").unwrap();
        // let mut url = Url::from_str("http://mono.mega.local:80").unwrap();
        let mut url = Url::from_str("http://172.17.0.1:32001").unwrap();
        let new_path = format!("/third-part/crates/{}", crate_name);
        url.set_path(&new_path);

        //println!("The URL is: {}", url);

        git_command_with_env()
            .arg("remote")
            .arg("remove")
            .arg("nju")
            .output()
            .unwrap();

        git_command_with_env()
            .arg("remote")
            .arg("add")
            .arg("nju")
            .arg(url.to_string())
            .output()
            .unwrap();

        //git push --set-upstream nju master
        let push_res = git_command_with_env()
            .arg("push")
            .arg("--set-upstream")
            .arg("nju")
            .arg("master")
            .output()
            .unwrap();

        git_command_with_env()
            .arg("push")
            .arg("nju")
            .arg("--tags")
            .output()
            .unwrap();

        record.mega_url = Set(url.path().to_owned());

        if push_res.status.success() {
            record.status = Set(RepoSyncStatus::Succeed);
            record.err_message = Set(None);
        } else {
            record.status = Set(RepoSyncStatus::Failed);
            record.err_message = Set(Some(String::from_utf8_lossy(&push_res.stderr).to_string()));
        }
        record.updated_at = Set(chrono::Utc::now().naive_utc());
        let res = record.save(conn).await.unwrap();
        let db_model: repo_sync_status::Model = res.try_into().unwrap();
        let kafka_message_model = message_model::MessageModel::new(
            db_model,                              // 数据库 Model
            message_model::MessageKind::Mega,      // 设置 message_kind 为 Mega
            message_model::SourceOfData::Cratesio, // 设置 source_of_data 为 Cratesio
            Utc::now(),                            // 当前时间作为时间戳
            "Extra information".to_string(),       // 设置 extra_field，示例中为一个字符串
        );
        println!("=================kafka_message{:?}", kafka_message_model);
        kafka::producer::send_message(
            producer,
            &env::var("KAFKA_TOPIC_NEW").unwrap(),
            serde_json::to_string(&kafka_message_model).unwrap(),
        )
        .await;
        println!("Push res: {}", String::from_utf8_lossy(&push_res.stdout));
        println!("Push err: {}", String::from_utf8_lossy(&push_res.stderr));
    }

    fn remove_extension(path: &Path) -> PathBuf {
        // Check if the path has an extension
        if let Some(stem) = path.file_stem() {
            // Create a new path without the extension
            if let Some(parent) = path.parent() {
                return parent.join(stem);
            }
        }
        // Return the original path if no extension was found
        path.to_path_buf()
    }

    fn decompress_crate_file(src: &Path, dst: &Path) -> io::Result<()> {
        // Open the source crate file
        let crate_file = File::open(src)?;
        // Create a GzDecoder to handle the gzip decompression
        let tar = GzDecoder::new(BufReader::new(crate_file));
        // Create a tar archive on top of the decompressed tarball
        let mut archive = Archive::new(tar);
        // Unpack the tar archive to the destination directory
        archive.unpack(dst)?;
        Ok(())
    }

    /// 从目录中读取最新的 `download_*.txt` 文件，解析出 crate 名集合
    fn read_latest_crate_list(dir: &Path) -> io::Result<HashSet<String>> {
        let mut latest_file: Option<PathBuf> = None;
        let mut latest_timestamp: Option<i64> = None;

        // 遍历目录下的文件
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            //println!("open txt {:?}", path);
            // 检查文件是否符合 `download_*.txt` 命名格式
            if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                if file_name.starts_with("download_") && file_name.ends_with(".txt") {
                    // 提取时间戳部分
                    let timestamp_str = file_name
                        .trim_start_matches("download_")
                        .trim_end_matches(".txt");

                    let parts: Vec<&str> = timestamp_str.split('_').collect();
                    let date_part = parts[0].replace('-', "");
                    let time_part = parts[1].replace('-', "");

                    // 拼接日期和时间部分
                    let formatted_str = format!("{}{}", date_part, time_part);
                    // 尝试将时间戳转换为数字
                    if let Ok(timestamp) = formatted_str.parse::<i64>() {
                        if latest_timestamp.is_none() || timestamp > latest_timestamp.unwrap() {
                            latest_timestamp = Some(timestamp);
                            latest_file = Some(path);
                        }
                    }
                }
            }
        }

        //println!("open txt {:?}", latest_file);
        // 如果找到最新文件，读取文件内容
        if let Some(latest_file_path) = latest_file {
            let file = fs::File::open(latest_file_path)?;
            let reader = BufReader::new(file);
            let mut crate_names = HashSet::new();

            for line in reader.lines() {
                let line = line?;
                crate_names.insert(line);
            }

            Ok(crate_names)
        } else {
            // 如果未找到符合条件的文件，则返回空集合
            Ok(HashSet::new())
        }
    }

    // 读取日志文件并解析出所需的信息
    fn parse_log_file(
        log_file_path: &Path,
        last_position: u64,
        crate_names: &HashSet<String>,
    ) -> io::Result<Vec<CrateInfo>> {
        let mut file = File::open(log_file_path)?;
        file.seek(SeekFrom::Start(last_position))?; // 从上次位置开始读取
        let reader = BufReader::new(file);

        let mut crates_info = Vec::new();
        /*for line_result in reader.lines() {
            let line = line_result?;
            for crate_name in crate_names {
                if line.contains("File { fd: ")
                    && line.contains("path: \"")
                    && line.contains(crate_name)
                {
                    let start = line.find("path: \"").unwrap() + "path: \"".len();
                    let end = line.rfind('"').unwrap();
                    let path_str = line[start..end].to_string();

                    let full_path = PathBuf::from(&path_str);
                    // Get the parent directory as the `path` you need
                    let path = full_path.parent().unwrap().to_path_buf();
                    let version = full_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .split('-')
                        .last()
                        .unwrap()
                        .replace(".crate", "");

                    crates_info.push(CrateInfo {
                        path,
                        full_path,
                        version,
                    });
                }
            }
        }*/

        let regex = Regex::new(r#"path: "([^"]+)""#).unwrap(); // 提取 path 字段的正则

        for line_result in reader.lines() {
            let line = line_result?;
            if line.contains("File { fd: ") && line.contains("path: \"") {
                // 使用正则提取路径
                if let Some(captures) = regex.captures(&line) {
                    let path_str = captures.get(1).unwrap().as_str();
                    let full_path = PathBuf::from(path_str);

                    // 获取文件名部分作为 crate 名进行校验
                    if let Some(file_name) = full_path.file_name() {
                        let file_name = file_name.to_str().unwrap();
                        let crate_name = full_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap();

                        //println!("Correct crate name: {}", crate_name); // 调试输出

                        // 确保 crate 名精确匹配
                        if crate_names.contains(crate_name) {
                            let path = full_path.parent().unwrap().to_path_buf();
                            let version =
                                file_name.split('-').last().unwrap().replace(".crate", "");

                            //println!("Correct crate version: {}", version); // 调试输出

                            crates_info.push(CrateInfo {
                                path,
                                full_path,
                                version,
                            });
                        }
                    }
                }
            }
        }

        // 使用 HashMap 按名称分组
        let mut grouped_crates: HashMap<String, Vec<CrateInfo>> = HashMap::new();
        for crate_info in crates_info {
            grouped_crates
                .entry(crate_info.path.to_str().unwrap().to_owned().clone())
                .or_default()
                .push(crate_info);
        }

        // 对每个组内的条目按版本号排序，然后将所有条目合并回一个 Vec
        let mut sorted_crates_info = Vec::new();
        for mut crate_list in grouped_crates.into_values() {
            crate_list.sort_by(|a, b| compare_versions(&a.version, &b.version));
            sorted_crates_info.extend(crate_list);
        }

        Ok(sorted_crates_info)
    }

    // 比较两个版本号的辅助函数
    fn compare_versions(version_a: &str, version_b: &str) -> Ordering {
        let parse_version = |version: &str| -> Vec<u32> {
            version
                .split('.')
                .filter_map(|v| v.parse::<u32>().ok())
                .collect()
        };

        let a_parts = parse_version(version_a);
        let b_parts = parse_version(version_b);

        a_parts.cmp(&b_parts)
    }

    fn git_command_with_env() -> Command {
        let mut cmd = Command::new("git");
        cmd.env_remove("http_proxy")
            .env_remove("https_proxy")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY");
        cmd
    }
}
