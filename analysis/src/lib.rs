pub mod db;
pub mod kafka_handler;
pub mod utils;

use kafka_handler::KafkaReader;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::io::{AsyncReadExt, BufReader};

use crate::db::get_dbhandler;
use crate::utils::{ensure_dir_exists, extract_namespace_and_path, run_command};

const TOOL_CONFIG_PATH: &str = "/var/tools/tools.json";

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

/// FIXME(hongwang):
/// 1. 这个函数本应实现一个通用的分析框架，能够根据tools.json动态适配不同的分析工具和命令，
///    但目前实现方式比较死板，所有命令参数和流程都写死在代码里，扩展性和灵活性较差。
/// 2. tools.json中的run字段本应支持任意命令模板，但现在只适配了gitleaks的固定命令，
///    没有真正做到通用化，导致后续增加新工具或命令时需要频繁修改代码。
/// 3. 代码结构上，命令拼接、输出处理、结果入库等逻辑都混杂在主流程里，
///    缺乏清晰的分层和可插拔机制，不利于维护和测试。
/// 4. 建议重构为：
///    - 支持tools.json中run字段为命令模板（如可用占位符），动态渲染参数
///    - 每个tool的执行、输出、入库逻辑可通过trait或回调自定义
///    - 主流程只负责调度和通用异常处理，具体细节交给tool实现
/// 5. 目前的实现方式导致tools.json的灵活性和可扩展性大打折扣，
///    违背了配置驱动和插件化的初衷。
#[allow(unused_variables)]
#[allow(clippy::needless_borrows_for_generic_args)]
#[allow(clippy::let_unit_value)]
pub async fn analyse_once(
    kafka_reader: &KafkaReader,
    output_path: &str,
) -> Result<(), Box<dyn Error>> {
    let config: Config = serde_json::from_str(&fs::read_to_string(TOOL_CONFIG_PATH)?)
        .expect("Failed to parse config");
    let tools = config.tools;

    let message = kafka_reader.read_single_message().await.unwrap();
    tracing::info!("Analysis receive {:?}", message);
    tracing::info!(
        "name:{},git_url:{:?}",
        message.db_model.crate_name,
        message.db_model.mega_url
    );
    let (namespace, repo_path) = extract_namespace_and_path(
        &message.db_model.mega_url,
        "/var/target/new_crates_file",
        &message.db_model.crate_name,
        None,
    )
    .await;

    tracing::info!("analyze namespace:{}", namespace.clone());

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
            ensure_dir_exists(&output_dir).await;
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
            let output = run_command(&mut cmd)
                .map_err(|e| format!("Failed to execute run command for {}: {}", tool.name, e))?;
            tracing::info!("output:{:?}", output);
            tracing::info!("finish command");
            let dbhandler = get_dbhandler().await;
            let id = namespace.clone();
            let file = tokio::fs::File::open(&output_file).await?;
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
    let (namespace, repo_path) = extract_namespace_and_path(
        &message.git_url,
        "/var/target/split_crates_file",
        &message.name,
        Some(&message.version),
    )
    .await;

    tracing::info!("analyze namespace:{}", namespace.clone());

    tracing::info!("code_path:{:?}", repo_path.clone());

    let dbhandler = get_dbhandler().await;
    let id = namespace.clone() + "/" + &message.name + "/" + &message.version;

    let mut clean_cmd = Command::new("cargo");
    clean_cmd.arg("clean").current_dir(&repo_path);
    run_command(&mut clean_cmd)
        .map_err(|e| format!("Failed to execute run command for : {}", e))?;
    tracing::info!("finish cargo clean");
    let mut mir_checker_cmd = Command::new("/workdir/cargo-mir-checker");
    mir_checker_cmd
        .arg("mir-checker")
        .arg("--")
        .arg("--show_entries")
        .current_dir(&repo_path);
    let output3 = run_command(&mut mir_checker_cmd).map_err(|e| {
        let _ = dbhandler.insert_mirchecker_failed_into_pg(id.clone());
        format!("Failed to execute run command for : {}", e)
    })?;
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
    for entry in entries {
        let mut clean_cmd = Command::new("cargo");
        clean_cmd.arg("clean").current_dir(&repo_path);
        run_command(&mut clean_cmd)
            .map_err(|e| format!("Failed to execute run command for : {}", e))?;
        let mut entry_cmd = Command::new("/workdir/cargo-mir-checker");
        entry_cmd
            .arg("mir-checker")
            .arg("--")
            .arg("--entry")
            .arg(&entry)
            .current_dir(&repo_path);
        let output4 = entry_cmd
            .output()
            .expect("Failed to execute cargo-mir-checker");
        if !output4.status.success() {
            let error_msg = String::from_utf8_lossy(&output4.stderr);
            tracing::info!("test entry {} Command failed: {}", entry.clone(), error_msg);
        }
        tracing::info!("entry: {},output: {:?}", entry.clone(), output4);
        let stderr_str = String::from_utf8_lossy(&output4.stderr);
        let mut warning_blocks = Vec::new();
        let mut current_block = String::new();
        let mut in_warning_block = false;
        for line in stderr_str.lines() {
            if line.starts_with("warning: [MirChecker]") {
                if in_warning_block && !current_block.is_empty() {
                    warning_blocks.push(current_block.clone());
                }
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
                } else {
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
        tracing::info!(
            "entry: {}, all mirchecker warning: {}",
            entry.clone(),
            combined_warnings.clone()
        );
        if !combined_warnings.is_empty() {
            all_outputs.push(combined_warnings.clone());
        }
    }
    let real_res = all_outputs.join("\n");
    let _ = dbhandler
        .insert_mirchecker_result_into_pg(id.clone(), real_res.clone())
        .await
        .unwrap();

    Ok(())
}
