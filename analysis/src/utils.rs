use std::path::{Path, PathBuf};
use std::process::Command;
use std::{
    env,
    fs::File,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing_subscriber::EnvFilter;

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
pub(crate) async fn extract_namespace(url_str: &str) -> Result<String, String> {
    tracing::info!("enter extract_namespace");
    /// auxiliary function
    fn remove_dot_git_suffix(input: &str) -> String {
        let input = if input.ends_with('/') {
            input.strip_suffix('/').unwrap()
        } else {
            input
        };

        let input = if input.ends_with(".git") {
            input.strip_suffix(".git").unwrap().to_string()
        } else {
            input.to_string()
        };
        input
    }

    let url = remove_dot_git_suffix(url_str);

    tracing::info!("finish get url:{:?}", url);
    // /tokio-rs/tokio

    let segments: Vec<&str> = url.split("/").collect();
    tracing::info!("finish get segments");

    // github URLs is of the format "/user/repo"
    if segments.len() < 2 {
        return Err(format!(
            "URL {} does not include a namespace and a repository name",
            url_str
        ));
    }

    // join owner name and repo name
    let namespace = format!(
        "{}/{}",
        segments[segments.len() - 2],
        segments[segments.len() - 1]
    );

    Ok(namespace)
}

// 通用：namespace和repo_path处理
pub async fn extract_namespace_and_path(
    url: &str,
    base_path: &str,
    name: &str,
    version: Option<&str>,
) -> (String, PathBuf) {
    let namespace = extract_namespace(url).await.unwrap();
    let repo_path = match version {
        Some(ver) => PathBuf::from(base_path)
            .join(&namespace)
            .join(format!("{}-{}", name, ver)),
        None => PathBuf::from(base_path).join(&namespace),
    };
    (namespace, repo_path)
}

#[allow(dead_code)]
pub fn init_git(repo_path: &str) -> Result<(), ()> {
    if let Err(e) = std::env::set_current_dir(Path::new(repo_path)) {
        println!("Failed to change directory: {}", e);
    } else {
        let init_output = Command::new("git")
            .arg("init")
            .output()
            .expect("Failed to execute git init");
        if !init_output.status.success() {
            let error_msg = String::from_utf8_lossy(&init_output.stderr);
            println!("git init failed: {}", error_msg);
        }
        let add_output = Command::new("git")
            .arg("add")
            .arg(".")
            .output()
            .expect("Failed to execute git add");
        if !add_output.status.success() {
            let error_msg = String::from_utf8_lossy(&add_output.stderr);
            println!("git add failed: {}", error_msg);
        }
        let commit_output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("first commit")
            .output()
            .expect("Failed to execute git commit");
        if !commit_output.status.success() {
            let error_msg = String::from_utf8_lossy(&commit_output.stderr);
            println!("git commit failed: {}", error_msg);
        }
    }
    Ok(())
}

pub fn init_logger(tool_name: &str) -> File {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let log_path = format!("/var/target/{}_{}_log.ans", tool_name, timestamp);
    let file = File::create(&log_path).expect("Unable to create log file");
    tracing_subscriber::fmt()
        .with_writer(file.try_clone().unwrap())
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    tracing::info!("Starting with log file: {}", log_path);
    file
}

pub fn load_env() -> (String, String, String) {
    dotenvy::dotenv().ok();
    let kafka_broker = env::var("KAFKA_BROKER").unwrap();
    let consumer_group_id = env::var("KAFKA_CONSUMER_GROUP_ID").unwrap();
    let analysis_topic = env::var("KAFKA_ANALYSIS_TOPIC").unwrap();
    tracing::debug!(
        "kafka_broker: {}, consumer_group_id: {}, analysis_topic: {}",
        kafka_broker,
        consumer_group_id,
        analysis_topic
    );
    (kafka_broker, consumer_group_id, analysis_topic)
}

// 通用：命令执行与错误处理
pub fn run_command(cmd: &mut Command) -> Result<std::process::Output, String> {
    let output = cmd.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(error_msg);
    }
    Ok(output)
}

// 通用：目录创建
pub async fn ensure_dir_exists(path: &Path) {
    if !path.is_dir() {
        let _ = tokio::fs::create_dir_all(path).await;
    }
}
