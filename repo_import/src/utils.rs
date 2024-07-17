use csv::Writer;
use lazy_static::lazy_static;
use model::tugraph_model::{Program, UProgram};
use serde::Serialize;
use serde_json::json;
use ssh2::Session;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt::Debug;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use url::Url;

lazy_static! {
    pub static ref NAMESPACE_HASHMAP: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

pub fn insert_namespace_by_repo_path(key: String, value: String) {
    let mut map = NAMESPACE_HASHMAP.lock().unwrap();
    map.insert(key, value);
}

pub fn get_namespace_by_repo_path(key: &str) -> Option<String> {
    let map = NAMESPACE_HASHMAP.lock().unwrap();
    map.get(key).cloned()
}

lazy_static! {
    pub static ref PROGRAM_HASHMAP: Mutex<HashMap<String, (Program, UProgram)>> =
        Mutex::new(HashMap::new());
}

pub fn insert_program_by_name(key: String, value: (Program, UProgram)) {
    let mut map = PROGRAM_HASHMAP.lock().unwrap();
    map.insert(key, value);
}

pub fn get_program_by_name(key: &str) -> Option<(Program, UProgram)> {
    let map = PROGRAM_HASHMAP.lock().unwrap();
    map.get(key).cloned()
}

pub(crate) fn write_into_csv<T: Serialize + Default + Debug>(
    csv_path: PathBuf,
    programs: Vec<T>,
) -> Result<(), Box<dyn Error>> {
    // open the csv

    let serialized = serde_json::to_value(&T::default()).unwrap();

    if let serde_json::Value::Object(map) = serialized {
        //let field_names: Vec<String> = map.keys().cloned().collect();
        let field_names: Vec<&str> = map.keys().map(|s| s.as_str()).collect();

        //debug!("{:?}", field_names);

        write_to_csv(field_names, csv_path.to_str().unwrap(), false)?;
    }

    for program in &programs {
        let fields = get_fields(program);
        let fields = fields.iter().map(|s| s.as_str()).collect::<Vec<_>>();

        //debug!("{:?}", fields);
        write_to_csv(fields, csv_path.to_str().unwrap(), true)?;
    }

    Ok(())
}

fn write_to_csv(data: Vec<&str>, file_path: &str, append: bool) -> Result<(), Box<dyn Error>> {
    let file = if append {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?
    } else {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)?
    };

    let mut wtr = Writer::from_writer(file);

    wtr.write_record(&data)?;

    wtr.flush()?;
    Ok(())
}

fn get_fields<T: Serialize>(item: &T) -> Vec<String> {
    let mut fields = Vec::new();
    let json = json!(item);
    if let serde_json::Value::Object(map) = json {
        fields = map
            .values()
            .map(|value| match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string().trim_matches('"').to_owned(),
            })
            .collect::<Vec<_>>();
    }
    fields
}

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
pub(crate) fn extract_namespace(url_str: &str) -> Result<String, String> {
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

    let url = Url::parse(&remove_dot_git_suffix(url_str))
        .map_err(|e| format!("Failed to parse URL {}: {}", url_str, e))?;

    // /tokio-rs/tokio
    let path_segments = url
        .path_segments()
        .ok_or("Cannot extract path segments from URL")?;

    let segments: Vec<&str> = path_segments.collect();
    //println!("{:?}", segments);

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

pub(crate) fn name_join_version(crate_name: &str, version: &str) -> String {
    crate_name.to_string() + "/" + version
}

pub async fn reset_mq() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(" Start reset Offset of Kafka.");
    let username = "rust";
    let password = &env::var("HOST_PASSWORD")?;
    let hostname = "172.17.0.1";
    let port = 22;

    tracing::trace!("xxxxxxxxx");

    // 连接到主机
    let tcp = TcpStream::connect((hostname, port))?;
    tracing::trace!("xxxxxxxxx");
    let mut sess = Session::new()?;

    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    tracing::trace!("xxxxxxxxx");

    // 使用用户名和密码进行身份验证
    sess.userauth_password(username, password)?;

    // 检查身份验证是否成功
    if !sess.authenticated() {
        panic!("Authentication failed!");
    }

    // 多行命令字符串
    let command = r#"
        docker exec pensive_villani /opt/kafka/bin/kafka-consumer-groups.sh \
        --bootstrap-server localhost:9092 \
        --group default_group \
        --reset-offsets \
        --to-offset 0 \
        --execute \
        --topic REPO_SYNC_STATUS.dev
    "#;

    // 运行命令
    let mut channel = sess.channel_session()?;
    channel.exec(command)?;

    // 读取命令输出
    let mut s = String::new();
    channel.read_to_string(&mut s)?;
    tracing::info!("Command output: {}", s);

    // 关闭通道
    channel.send_eof()?;
    channel.wait_close()?;
    tracing::info!(
        "Finish reset Kafka MQ, Exit status: {}",
        channel.exit_status()?
    );

    Ok(())
}
