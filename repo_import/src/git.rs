use crates_sync::{init::database_connection, query::MegaStorage};
use git2::{build::CheckoutBuilder, ObjectType, Repository};
//use rayon::prelude::*;
use std::{path::PathBuf, sync::Arc};
use url::Url;

use crate::info;

/// clone repo locally
/// 1. Get mega url from postgres
/// 2. Clone git repositories from mega, reserving the namespace as path where they are cloned
pub(crate) async fn clone_repos_from_pg(
    mega_url_base: &str,
    clone_dir: &str,
) -> Result<(), String> {
    // read from postgres sql
    let database_conn = database_connection().await;
    let repo_sync: MegaStorage = MegaStorage::new(Arc::new(database_conn));
    let mut krates: Vec<crates_sync::repo_sync_model::RepoSync> = repo_sync.get_all_repos().await;
    krates.sort_by_key(|x| x.mega_url.clone());
    let krates: Vec<&crates_sync::repo_sync_model::RepoSync> = krates.iter().take(10).collect();

    // rayon parallel iter, make it faster
    krates.iter().for_each(|krate| {
        //krates.par_iter().for_each(|krate| {
        let mega_url = &krate.mega_url;
        // FIXME:
        let mega_url_base = Url::parse(mega_url_base)
            .unwrap_or_else(|_| panic!("Failed to parse mega url base: {}", &mega_url_base));
        let mega_url = mega_url_base
            .join(mega_url)
            .expect("Failed to join url path");

        let namespace = remove_dot_git_suffix(
            &extract_namespace(mega_url.as_ref()).expect("Failed to parse URL"),
        );

        let path = PathBuf::from(clone_dir).join(namespace);

        clone(&path, mega_url.as_ref());
    });

    Ok(())
}

fn clone(path: &PathBuf, url: &str) {
    if !path.is_dir() {
        info!("Cloning repo into {:?} from URL {}", path, url);
        match Repository::clone(url, path) {
            Ok(_) => info!("Successfully cloned into {:?}", path),
            Err(e) => panic!("Failed to clone {}: {:?}", url, e),
        }
    } else {
        warn!("Directory {:?} is not empty, skipping clone", path);
    }
}

pub(crate) fn hard_reset_to_head(repo: &Repository) -> Result<(), git2::Error> {
    // 获取当前HEAD指向的提交
    let head = repo.head()?;
    let commit = repo.find_commit(
        head.target()
            .ok_or(git2::Error::from_str("HEAD does not point to a commit"))?,
    )?;

    // 获取当前提交的树
    let tree = commit.tree()?;

    // 创建CheckoutBuilder，设置为强制检出，以确保工作目录的变更
    let mut checkout_opts = CheckoutBuilder::new();
    checkout_opts.force();

    // 正确地将tree转换为Object再进行检出
    let tree_obj = tree.into_object();
    repo.checkout_tree(&tree_obj as &git2::Object, Some(&mut checkout_opts))?;
    Ok(())
}

pub(crate) fn print_all_tags(repo: &Repository) {
    let tags = repo.tag_names(None).unwrap();
    for tag_name in tags.iter().flatten() {
        let tag_ref = repo
            .find_reference(&format!("refs/tags/{}", tag_name))
            .unwrap();
        // 解析标签指向的对象
        if let Ok(tag_object) = tag_ref.peel_to_tag() {
            // Annotated 标签
            let target_commit = tag_object.target().unwrap().peel_to_commit().unwrap();
            println!(
                "Annotated Tag: {}, Commit: {}, Message: {}",
                tag_name,
                target_commit.id(),
                tag_object.message().unwrap_or("No message")
            );
        } else {
            // 轻量级标签可能不能直接转换为 annotated 标签对象
            // 直接获取引用指向的提交
            let commit_object = tag_ref.peel(ObjectType::Commit).unwrap();
            let commit = commit_object
                .into_commit()
                .expect("Failed to peel into commit");
            println!("Lightweight Tag: {}, Commit: {}", tag_name, commit.id());
            // 轻量级标签没有存储消息
        }
    }
}

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
fn extract_namespace(url_str: &str) -> Result<String, String> {
    let url = Url::parse(url_str).map_err(|e| format!("Failed to parse URL {}: {}", url_str, e))?;

    // /tokio-rs/tokio
    let path_segments = url
        .path_segments()
        .ok_or("Cannot extract path segments from URL")?;

    let segments: Vec<&str> = path_segments.collect();

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

/// auxiliary function
fn remove_dot_git_suffix(input: &str) -> String {
    if input.ends_with(".git") {
        input.replace(".git", "")
    } else {
        input.to_string()
    }
}
