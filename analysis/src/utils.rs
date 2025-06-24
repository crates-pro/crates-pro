use std::path::Path;
use std::process::Command;

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
