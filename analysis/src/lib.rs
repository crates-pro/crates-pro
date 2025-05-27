pub mod kafka_handler;
mod utils;

use kafka_handler::KafkaReader;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::io::{AsyncReadExt, BufReader};
use tokio_postgres::NoTls;

use data_transporter::db::{db_connection_config_from_env, DBHandler};
#[allow(dead_code)]
#[derive(Deserialize)]
struct ToolConfig {
    name: String, //name
    binary_path: String,
    run: Vec<String>, // how to run
}

#[derive(Deserialize)]
struct Config {
    tools: Vec<ToolConfig>,
}
#[allow(unused_variables)]
#[allow(clippy::needless_borrows_for_generic_args)]
#[allow(clippy::let_unit_value)]
/// Input: a message with version
/// output: a file
pub async fn analyse_once(
    kafka_reader: &KafkaReader,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let config_path = Path::new("/var/tools/tools.json");
    let config: Config =
        serde_json::from_str(&fs::read_to_string(config_path)?).expect("Failed to parse config");

    let tools = config.tools;

    let message = kafka_reader.read_single_message().await.unwrap();
    tracing::info!("Analysis receive {:?}", message);
    tracing::info!(
        "name:{},git_url:{:?}",
        message.db_model.crate_name,
        message.db_model.mega_url
    );
    let namespace = utils::extract_namespace(&message.db_model.mega_url).await?;

    tracing::info!("analyze namespace:{}", namespace.clone());

    let repo_path = PathBuf::from("/var/target/new_crates_file").join(&namespace);
    tracing::info!("code_path:{:?}", repo_path.clone());

    for tool in &tools {
        for command in &tool.run {
            let output_file = PathBuf::from(output_path)
                .join(&tool.name)
                .join(&namespace)
                .join(message.db_model.crate_name.clone() + ".txt");
            let output_dir = PathBuf::from(output_path).join(&tool.name).join(&namespace);

            tracing::info!("output_file_path:{:?}", output_file.clone());
            tracing::info!("output_dir:{:?}", output_dir.clone());
            if !output_dir.is_dir() {
                let _ = tokio::fs::create_dir_all(&output_dir).await;
            }
            let f = tokio::fs::File::create(&output_file).await.unwrap();

            let gitleaks = PathBuf::from("/var/tools/sensleak/gitleaks.toml");
            let mut cmd = Command::new("/var/tools/sensleak/scan");
            cmd.args(&[
                "--repo",
                repo_path.to_str().unwrap(),
                "--config",
                gitleaks.to_str().unwrap(),
                "-v",
                "--pretty",
                "--report",
                output_file.to_str().unwrap(),
            ]);
            let output = cmd.output()?;
            tracing::info!("output:{:?}", output);
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                tracing::info!("Command failed with error: {}", error_msg);
                return Err(format!(
                    "Failed to execute run command for {}: {}",
                    tool.name, error_msg
                )
                .into());
            }
            tracing::info!("finish command");
            //insert into pg
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
            let id = namespace.clone();
            let file = tokio::fs::File::open(output_file).await?;
            let mut reader = BufReader::new(file);
            let mut content = String::new();
            reader.read_to_string(&mut content).await?;
            tracing::info!("content:{}", content.clone());
            let _ = dbhandler
                .insert_sensleak_result_into_pg(id.clone(), content.clone())
                .await
                .unwrap();
        }
    }

    Ok(())
}
#[allow(dead_code)]
fn init_git(repo_path: &str) -> Result<(), ()> {
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
#[allow(unused_variables)]
#[allow(clippy::needless_borrows_for_generic_args)]
#[allow(clippy::let_unit_value)]
/// Input: a message with version
/// output: a file
pub async fn analyse_once_mirchecker(
    kafka_reader: &KafkaReader,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {

    let message = kafka_reader.read_single_message_mirchecker().await.unwrap();
    tracing::info!("Analysis receive {:?}", message);
    tracing::info!(
        "name:{},git_url:{:?}",
        message.name.clone(),
        message.git_url.clone()
    );
    let namespace = utils::extract_namespace(&message.git_url).await?;

    tracing::info!("analyze namespace:{}", namespace.clone());

    let repo_path = PathBuf::from("/var/target/split_crates_file")
        .join(&namespace)
        .join(message.name.clone() + "-" + &message.version);
    tracing::info!("code_path:{:?}", repo_path.clone());

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
            let id = namespace.clone() + "/" + &message.name + "/" + &message.version;
            
            let output2 = Command::new("cargo")
            .arg("clean")
            .current_dir(&repo_path)  // 指定工作目录
            .output()
            .expect("Failed to cargo clean");
            if !output2.status.success() {
                let error_msg = String::from_utf8_lossy(&output2.stderr);
                tracing::info!("cargo clean Command failed with error: {}", error_msg);
                return Err(format!(
                    "Failed to execute run command for : {}",
                     error_msg
                )
                .into());
            }
            tracing::info!("finish cargo clean");
            let output3 = Command::new("/workdir/cargo-mir-checker")
            .arg("mir-checker")
            .arg("--")
            .arg("--show_entries")
            .current_dir(&repo_path)  // 指定工作目录
            .output()
            .expect("Failed to execute cargo-mir-checker");
            if !output3.status.success() {
                let error_msg = String::from_utf8_lossy(&output3.stderr);
                tracing::info!("show entry Command failed ");
                let _ = dbhandler
                .insert_mirchecker_failed_into_pg(id.clone())
                .await
                .unwrap();
                return Err(format!(
                    "Failed to execute run command for : {}",
                     error_msg
                )
                .into());
            }
            tracing::info!("start get stdout_str");
            let stdout_str = String::from_utf8(output3.stdout)?;
            tracing::info!("finish get stdout_str");
            let entries: Vec<String> = stdout_str
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();
            tracing::info!("show entries success:");
            tracing::info!("finish show entries");
            let mut all_outputs = vec![];
            for entry in entries{
                let output3 = Command::new("cargo")
                .arg("clean")
                .current_dir(&repo_path)  // 指定工作目录
                .output()
                .expect("Failed to cargo clean");
                if !output3.status.success() {
                    let error_msg = String::from_utf8_lossy(&output3.stderr);
                    tracing::info!("cargo clean Command failed with error: {}", error_msg);
                    return Err(format!(
                        "Failed to execute run command for : {}",
                        error_msg
                    )
                    .into());
                }
                let output4 = Command::new("/workdir/cargo-mir-checker")
                .arg("mir-checker")
                .arg("--")
                .arg("--entry")
                .arg(&entry)
                .current_dir(&repo_path)  // 指定工作目录
                .output()
                .expect("Failed to execute cargo-mir-checker");
                if !output4.status.success() {
                    let error_msg = String::from_utf8_lossy(&output4.stderr);
                    tracing::info!("test entry {} Command failed: {}",entry.clone(),error_msg);
                }
                tracing::info!("entry: {},output: {:?}",entry.clone(),output4);
                let stderr_str = String::from_utf8_lossy(&output4.stderr);
                let mut warning_blocks = Vec::new();
                let mut current_block = String::new();
                let mut in_warning_block = false;
                for line in stderr_str.lines() {
                    if line.starts_with("warning: [MirChecker]") {
                        // 保存已收集的块（如果有）
                        if in_warning_block && !current_block.is_empty() {
                            warning_blocks.push(current_block.clone());
                        }
                        // 开始新的块
                        in_warning_block = true;
                        current_block.clear();
                        current_block.push_str(line);
                        current_block.push('\n');
                    } else if in_warning_block {
                        if line.starts_with(" INFO") {
                            if !current_block.is_empty() {
                                warning_blocks.push(current_block.clone());
                            }
                            current_block.clear();
                            in_warning_block = false;
                        }
                        else{
                            current_block.push_str(line);
                            current_block.push('\n');
                        }
                    }
                }
                if in_warning_block && !current_block.is_empty() {
                    warning_blocks.push(current_block);
                }
                if !warning_blocks.is_empty() {
                    tracing::info!("共提取了 {} 个警告块:", warning_blocks.len());
                    for (i, block) in warning_blocks.iter().enumerate() {
                        tracing::info!("警告块 {}:\n{}", i + 1, block);
                    }
                } else {
                    tracing::info!("未找到符合条件的警告块");
                }
                let combined_warnings: String = warning_blocks.join("\n");
                tracing::info!("entry: {}, all mirchecker warning: {}",entry.clone(),combined_warnings.clone());
                if !combined_warnings.is_empty(){
                    all_outputs.push(combined_warnings.clone());
                }
            }
            //insert into pg
            let real_res = all_outputs.join("\n");

            let _ = dbhandler
                .insert_mirchecker_result_into_pg(id.clone(), real_res.clone())
                .await
                .unwrap();
        

    Ok(())
}
