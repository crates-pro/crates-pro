use crates_sync::{init::database_connection, query::MegaStorage};
use git2::Repository;
use rayon::prelude::*;
use std::{path::PathBuf, sync::Arc};
use url::Url;

/// clone repo locally
/// 1. Get mega url from postgress
/// 2. Clone git repositories from mega, reserving the namespace as path where they are cloned
pub(crate) async fn clone_repos_from_pg(clone_dir: &str) -> Result<(), String> {
    let database_conn = database_connection().await;
    let repo_sync: MegaStorage = MegaStorage::new(Arc::new(database_conn));
    let krates: Vec<crates_sync::repo_sync_model::RepoSync> = repo_sync.get_all_repos().await;

    // rayon parallel iter, make it faster
    krates.par_iter().for_each(|krate| {
        let mega_url = &krate.mega_url;
        let namespace = &extract_namespace(mega_url).expect("Failed to parse URL");
        let path = PathBuf::from(clone_dir).join(namespace);
        println!("Cloning repo into {:?} from URL {}", path, mega_url);
        if Repository::clone(mega_url, clone_dir).is_ok() {
            println!("Successfully cloned {}", mega_url);
        } else {
            println!("Failed to clone {}", mega_url);
        }
    });
    Ok(())
}

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
fn extract_namespace(url_str: &str) -> Result<String, String> {
    let url = Url::parse(url_str).map_err(|_| "Failed to parse URL")?;

    // /tokio-rs/tokio
    let path_segments = url
        .path_segments()
        .ok_or("Cannot extract path segments from URL")?;

    let segments: Vec<&str> = path_segments.collect();

    // github URLs is of the format "/user/repo"
    if segments.len() != 2 {
        return Err(format!(
            "URL {} does not include a namespace and a repository name",
            url_str
        ));
    }

    // join owner name and repo name
    let namespace = format!("{}/{}", segments[0], segments[1]);
    Ok(namespace)
}
