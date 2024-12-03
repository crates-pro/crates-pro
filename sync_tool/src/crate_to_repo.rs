use std::{
    env,
    fs::{self, File},
    io::{self, BufReader},
    path::{Path, PathBuf},
    process::{exit, Command},
    str::FromStr,
};

use flate2::bufread::GzDecoder;
use git2::{Repository, Signature};
use rdkafka::producer::FutureProducer;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, Unchanged};
use tar::Archive;
use url::Url;
use walkdir::WalkDir;

use entity::{db_enums::RepoSyncStatus, repo_sync_status};

use crate::{
    kafka::{self},
    util,
};

pub async fn convert_crate_to_repo(workspace: PathBuf) {
    let conn = util::db_connection().await;
    let producer = kafka::get_producer();

    for crate_entry in WalkDir::new(workspace)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if crate_entry.path().is_dir() {
            println!("re: {:?}", crate_entry);
            let crate_path = crate_entry.path();
            let crate_name = crate_path.file_name().unwrap().to_str().unwrap();
            let repo_path = &crate_path.join(crate_name);

            let record = crate::get_record(&conn, crate_name).await;
            if record.status == Unchanged(RepoSyncStatus::Succeed) {
                tracing::info!("skipping:{:?}", record.crate_name);
                // let kafka_payload: repo_sync_status::Model = record.try_into().unwrap();
                // kafka::producer::send_message(
                //     &producer,
                //     &env::var("KAFKA_TOPIC").unwrap(),
                //     serde_json::to_string(&kafka_payload).unwrap(),
                // )
                // .await;
                continue;
            }

            if repo_path.exists() {
                fs::remove_dir_all(repo_path).unwrap();
            }

            let mut crate_versions: Vec<PathBuf> = WalkDir::new(crate_path)
                .min_depth(1)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file() && e.path().extension().unwrap_or_default() == "crate"
                })
                .map(|entry| entry.path().to_path_buf())
                .collect();
            crate_versions.sort();

            for crate_v in crate_versions {
                let repo = open_or_make_repo(repo_path);

                decompress_crate_file(&crate_v, crate_entry.path()).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                });

                let uncompress_path = remove_extension(&crate_v);
                if fs::read_dir(&uncompress_path).is_err() {
                    continue;
                }
                empty_folder(repo.workdir().unwrap()).unwrap();
                copy_all_files(&uncompress_path, repo.workdir().unwrap()).unwrap();

                let version = &crate_v
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace(&format!("{}-", crate_name), "")
                    .replace(".crate", "");
                add_and_commit(&repo, version, repo_path);
                fs::remove_dir_all(uncompress_path).unwrap();
            }

            if repo_path.exists() {
                push_to_remote(&conn, crate_name, repo_path, record, &producer).await;
            } else {
                eprintln!("empty crates directory:{:?}", crate_entry.path())
            }
        }
    }

    fn open_or_make_repo(repo_path: &Path) -> Repository {
        match Repository::open(repo_path) {
            Ok(repo) => repo,
            Err(_) => {
                println!("Creating a new repository...");
                // Create a new repository
                match Repository::init(repo_path) {
                    Ok(repo) => {
                        println!(
                            "Successfully created a new repository at: {}",
                            repo_path.display()
                        );
                        repo
                    }
                    Err(e) => {
                        panic!("Failed to create a new repository: {}", e);
                    }
                }
            }
        }
    }

    fn add_and_commit(repo: &Repository, version: &str, repo_path: &Path) {
        if let Err(err) = env::set_current_dir(repo_path) {
            eprintln!("Error changing directory: {}", err);
            exit(1);
        }
        // Add all changes in the working directory to the index
        Command::new("git").arg("add").arg("./").output().unwrap();

        // Get the repository index
        let mut index = repo.index().unwrap();

        index.write().unwrap();

        // Create a tree from the index
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        // Get the current HEAD commit (if any)
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit().unwrap()),
            Err(_) => None,
        };

        // Create a signature
        let sig = Signature::now("Mega", "admin@mega.com").unwrap();

        // Create a new commit
        let commit_id = if let Some(parent) = parent_commit {
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit Version: {}", version),
                &tree,
                &[&parent],
            )
            .unwrap()
        } else {
            // Initial commit (no parents)
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit Version: {}", version),
                &tree,
                &[],
            )
            .unwrap()
        };

        // Create the tag
        repo.tag_lightweight(version, &repo.find_object(commit_id, None).unwrap(), false)
            .unwrap();
    }

    fn copy_all_files(src: &Path, dst: &Path) -> io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }

        for entry in fs::read_dir(src).unwrap() {
            let entry = entry?;
            let path = entry.path();
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };
            let dest_path = dst.join(file_name);

            if path.is_dir() {
                if !path.ends_with(".git") {
                    copy_all_files(&path, &dest_path).unwrap();
                }
            } else {
                fs::copy(&path, &dest_path).unwrap();
            }
        }
        Ok(())
    }

    fn empty_folder(dir: &Path) -> io::Result<()> {
        for entry in WalkDir::new(dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                fs::remove_file(path)?;
            } else if path.is_dir() && path.file_name().unwrap() != ".git" {
                fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

    async fn push_to_remote(
        conn: &DatabaseConnection,
        crate_name: &str,
        repo_path: &Path,
        mut record: repo_sync_status::ActiveModel,
        producer: &FutureProducer,
    ) {
        if let Err(err) = env::set_current_dir(repo_path) {
            eprintln!("Error changing directory: {}", err);
            exit(1);
        }

        let mut url = Url::from_str("http://localhost:8000").unwrap();
        let new_path = format!("/third-part/crates/{}", crate_name);
        url.set_path(&new_path);

        Command::new("git")
            .arg("remote")
            .arg("remove")
            .arg("nju")
            .output()
            .unwrap();

        Command::new("git")
            .arg("remote")
            .arg("add")
            .arg("nju")
            .arg(url.to_string())
            .output()
            .unwrap();

        //git push --set-upstream nju main
        let push_res = Command::new("git")
            .arg("push")
            .arg("--set-upstream")
            .arg("nju")
            .arg("main")
            .output()
            .unwrap();

        Command::new("git")
            .arg("push")
            .arg("nju")
            .arg("--tags")
            .output()
            .unwrap();

        record.mega_url = Set(url.path().to_owned());

        if push_res.status.success() {
            record.status = Set(RepoSyncStatus::Succeed);
            record.err_message = Set(None);
        } else {
            record.status = Set(RepoSyncStatus::Failed);
            record.err_message = Set(Some(String::from_utf8_lossy(&push_res.stderr).to_string()));
        }
        record.updated_at = Set(chrono::Utc::now().naive_utc());
        let res = record.save(conn).await.unwrap();
        let kafka_payload: repo_sync_status::Model = res.try_into().unwrap();
        kafka::producer::send_message(
            producer,
            &env::var("KAFKA_TOPIC").unwrap(),
            serde_json::to_string(&kafka_payload).unwrap(),
        )
        .await;
        println!("Push res: {}", String::from_utf8_lossy(&push_res.stdout));
        println!("Push err: {}", String::from_utf8_lossy(&push_res.stderr));
    }

    fn remove_extension(path: &Path) -> PathBuf {
        // Check if the path has an extension
        if let Some(stem) = path.file_stem() {
            // Create a new path without the extension
            if let Some(parent) = path.parent() {
                return parent.join(stem);
            }
        }
        // Return the original path if no extension was found
        path.to_path_buf()
    }

    fn decompress_crate_file(src: &Path, dst: &Path) -> io::Result<()> {
        // Open the source crate file
        let crate_file = File::open(src)?;
        // Create a GzDecoder to handle the gzip decompression
        let tar = GzDecoder::new(BufReader::new(crate_file));
        // Create a tar archive on top of the decompressed tarball
        let mut archive = Archive::new(tar);
        // Unpack the tar archive to the destination directory
        archive.unpack(dst)?;
        Ok(())
    }
}
