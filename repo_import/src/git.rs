use git2::{build::CheckoutBuilder, ObjectType, Repository};
use std::path::PathBuf;
use url::Url;

use crate::{utils::extract_namespace, utils::insert_namespace_by_repo_path, ImportDriver};

impl ImportDriver {
    /// clone repo locally
    /// 1. Get mega url from postgres
    /// 2. Clone git repositories from mega, reserving the namespace as path where they are cloned
    pub(crate) async fn clone_repos_from_pg(
        &mut self,
        mega_url_base: &str,
        clone_dir: &str,
    ) -> Result<(), String> {
        // read from postgres sql

        // TODO: MQ to get crates

        let mut krates: Vec<crates_sync::repo_sync_model::Model> = vec![];

        krates.sort_by_key(|x| x.mega_url.clone());

        // FIXME: test code
        let krates: Vec<&crates_sync::repo_sync_model::Model> = krates.iter().take(500).collect();

        // rayon parallel iter, make it faster
        krates.iter().for_each(|krate| {
            // use rayon::prelude::*;
            // krates.par_iter().for_each(|krate| {

            // mega_url = base + path
            let mega_url = {
                let mega_url_base = Url::parse(mega_url_base).unwrap_or_else(|_| {
                    panic!("Failed to parse mega url base: {}", &mega_url_base)
                });
                let mega_url_path = &krate.mega_url;
                mega_url_base
                    .join(mega_url_path)
                    .expect("Failed to join url path")
            };

            // namespace such as tokio-rs/tokio
            let namespace = extract_namespace(mega_url.as_ref()).expect("Failed to parse URL");

            // The path the repo will be cloned into
            let path = PathBuf::from(clone_dir).join(namespace.clone());

            if !self.cli.dont_clone {
                self.clone(&path, mega_url.as_ref());
            }
            // finish cloning, store namespace ...

            insert_namespace_by_repo_path(path.to_str().unwrap().to_string(), namespace.clone());
        });

        trace!("Finish clone all the repos\n");

        Ok(())
    }

    fn clone(&self, path: &PathBuf, url: &str) {
        println!("Repo into {:?} from URL {}", path, url);
        if !path.is_dir() {
            info!("Cloning repo into {:?} from URL {}", path, url);
            match Repository::clone(url, path) {
                Ok(_) => info!("Successfully cloned into {:?}", path),
                Err(e) => error!("Failed to clone {}: {:?}", url, e),
            }
        } else {
            warn!("Directory {:?} is not empty, skipping clone", path);
        }
    }
}

/// Deprecated.

/// If it migrate from a different system,
/// the git record will change, and this is the reset function.
pub(crate) fn hard_reset_to_head(repo: &Repository) -> Result<(), git2::Error> {
    let head = repo.head()?;
    let commit = repo.find_commit(
        head.target()
            .ok_or(git2::Error::from_str("HEAD does not point to a commit"))?,
    )?;

    // commit tree
    let tree = commit.tree()?;

    // Create CheckoutBuilder, set to force checkout to ensure changes to the working directory
    let mut checkout_opts = CheckoutBuilder::new();
    checkout_opts.force();

    // Correctly convert tree to Object before checking out the
    let tree_obj = tree.into_object();
    repo.checkout_tree(&tree_obj as &git2::Object, Some(&mut checkout_opts))?;
    Ok(())
}

pub(crate) fn _print_all_tags(repo: &Repository, v: bool) {
    let tags = repo.tag_names(None).unwrap();

    // for tag in tags.iter() {
    //     println!("tags: {}", tag.unwrap());
    // }

    let mut s = "".to_string();
    for tag_name in tags.iter().flatten() {
        let tag_ref = repo
            .find_reference(&format!("refs/tags/{}", tag_name))
            .unwrap();

        if v {
            if let Ok(tag_object) = tag_ref.peel_to_tag() {
                // Annotated tag
                let target_commit = tag_object.target().unwrap().peel_to_commit().unwrap();
                debug!(
                    "Annotated Tag: {}, Commit: {}, Message: {}",
                    tag_name,
                    target_commit.id(),
                    tag_object.message().unwrap_or("No message")
                );
            } else {
                let commit_object = tag_ref.peel(ObjectType::Commit).unwrap();
                let commit = commit_object
                    .into_commit()
                    .expect("Failed to peel into commit");
                debug!("Lightweight Tag: {}, Commit: {}", tag_name, commit.id());
            }
        } else {
            s += &format!("{}, ", tag_name);
        }
    }

    debug!("TAGS {:?} tags: {}", repo.path(), s);
}
