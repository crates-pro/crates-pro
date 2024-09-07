mod kafka_handler;
mod utils;

use kafka_handler::KafkaReader;
use serde::Deserialize;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};
use tempfile::tempdir;

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

/// Input: a message with version
/// output: a file
pub async fn analyse_once(output_path: &str) -> Result<(), Box<dyn Error>> {
    let config_path = Path::new("tools/tools.json");
    let config: Config =
        serde_json::from_str(&fs::read_to_string(config_path)?).expect("Failed to parse config");
    let tools = config.tools;

    let kafka_broker = env::var("KAFKA_BROKER")?;
    let kafka_group_id = env::var("KAFKA_GROUP_ID")?;
    let kafka_topic = env::var("KAFKA_ANALYSIS_TOPIC")?;

    let kafka_reader = KafkaReader::new(&kafka_broker, &kafka_group_id);

    let message = kafka_reader
        .read_single_message(&kafka_topic)
        .ok_or("No message received")?;
    tracing::info!("Analysis receive {:?}", message);

    let namespace = utils::extract_namespace(&message.git_url).await?;

    let tmp_dir = tempdir()?;
    let repo_path = tmp_dir.path().join("repo");

    // Clone the repository
    let clone_status = Command::new("git")
        .args(["clone", &message.git_url, repo_path.to_str().unwrap()])
        .status()?;

    if !clone_status.success() {
        return Err(format!("Failed to clone repository: {}", &message.git_url).into());
    }

    // Checkout the specific tag
    let checkout_status = Command::new("git")
        .arg("-C")
        .arg(repo_path.to_str().unwrap())
        .args(["checkout", &message.tag])
        .status()?;

    if !checkout_status.success() {
        return Err(format!("Failed to checkout tag: {}", &message.tag).into());
    }

    for tool in &tools {
        for command in &tool.run {
            let output_file = PathBuf::from(output_path)
                .join(&namespace)
                .join(message.version.clone() + "-" + &tool.name);

            let run_command = command
                .replace("{name}", &tool.name)
                .replace("{binary_path}", &tool.binary_path)
                .replace("{code_path}", repo_path.to_str().unwrap())
                .replace("{output_path}", output_file.to_str().unwrap());
            println!("Executing: {}", run_command);
            let run_status = Command::new("sh").arg("-c").arg(&run_command).status()?;

            if !run_status.success() {
                return Err(format!("Failed to execute run command for {}", tool.name).into());
            }
        }
    }

    Ok(())
}
