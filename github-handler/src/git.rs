use std::path::{Path, PathBuf};

use tokio::process::Command as TokioCommand;
use tracing::{debug, info, warn};

pub async fn clone_repo(
    target_dir: &Path,
    owner: &str,
    repo: &str,
    partial_clone: bool,
) -> Result<(), anyhow::Error> {
    debug!("克隆仓库到指定目录: {}", target_dir.display());
    let clone_url = format!("https://github.com/{}/{}.git", owner, repo);
    let path = target_dir.to_string_lossy();
    let mut args = vec![
        "clone",
        "--no-checkout",
        "--config",
        "credential.helper=reject", // 拒绝认证请求，不会提示输入
        "--config",
        "http.lowSpeedLimit=1000", // 设置低速限制
        "--config",
        "http.lowSpeedTime=10", // 如果速度低于限制持续10秒则失败
        "--config",
        "core.askpass=echo", // 不使用交互式密码提示
        &clone_url,
        &path,
    ];
    if partial_clone {
        args.push("--filter=blob:none");
    }
    let status = TokioCommand::new("git").args(args).status().await;

    match status {
        Ok(status) if !status.success() => {
            warn!("克隆仓库失败: {}，可能需要认证或不存在，跳过此仓库", status);
            return Ok(());
        }
        Err(e) => {
            warn!("执行git命令失败: {}，跳过此仓库", e);
            return Ok(());
        }
        _ => {}
    }
    Ok(())
}

pub fn is_shallow_repo(path: &PathBuf) -> bool {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(path)
        .output()
        .expect("Failed to run git");

    String::from_utf8_lossy(&output.stdout).trim() == "true"
}

pub async fn update_repo(
    target_dir: &PathBuf,
    owner: &str,
    repo: &str,
) -> Result<(), anyhow::Error> {
    info!("更新之前clone的仓库: {}", target_dir.display());

    TokioCommand::new("git")
        .current_dir(target_dir)
        .args(["reset", "--hard", "HEAD"])
        .status()
        .await?;

    TokioCommand::new("git")
        .current_dir(target_dir)
        .args(["clean", "-fd"])
        .status()
        .await?;

    let args = vec![
        "-c",
        "credential.helper=reject",
        "-c",
        "http.lowSpeedLimit=1000",
        "-c",
        "http.lowSpeedTime=10",
        "-c",
        "core.askpass=echo",
        "pull",
        "--rebase",
    ];
    let status = TokioCommand::new("git")
        .current_dir(target_dir)
        .args(args)
        .status()
        .await;
    match status {
        Ok(status) => {
            if !status.success() {
                eprintln!("Git command failed with status: {:?}", status);
                std::fs::remove_dir_all(target_dir)?;
                clone_repo(target_dir, owner, repo, false).await?;
            }
        }
        Err(e) => {
            eprintln!("Error executing git command: {}", e);
        }
    }
    Ok(())
}

pub async fn restore_shallow_repo(target_dir: &PathBuf) -> Result<(), anyhow::Error> {
    info!("恢复clone的shallow仓库: {}", target_dir.display());
    let output = TokioCommand::new("git")
        .current_dir(target_dir)
        .args(["remote", "show", "origin"])
        .env("LANG", "en_US.UTF-8") // 设置输出语言为英文
        .output()
        .await
        .ok()
        .unwrap();

    let stdout = std::str::from_utf8(&output.stdout).ok().unwrap();

    for line in stdout.lines() {
        if line.trim_start().starts_with("HEAD branch:") {
            let default_branch = line
                .split(':')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap();
            let args = vec![
                "-c",
                "credential.helper=reject",
                "-c",
                "http.lowSpeedLimit=1000",
                "-c",
                "http.lowSpeedTime=10",
                "-c",
                "core.askpass=echo",
                "checkout",
                &default_branch,
            ];
            let status = TokioCommand::new("git")
                .current_dir(target_dir)
                .args(args)
                .status()
                .await;
            if let Err(e) = status {
                warn!("更新仓库失败: {}，可能需要认证，继续分析当前代码", e);
            }
        }
    }

    Ok(())
}
