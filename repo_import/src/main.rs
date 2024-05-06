mod db;
mod dep;
mod git;
mod info;

use crate::info::extract_info_local;
use crate::info::write_into_csv;
use crate::{dep::extract_all_tags, git::print_all_tags};
use db::clone_repos_from_pg;
use git::hard_reset_to_head;
use git2::Repository;
use model::crate_info::*;
use std::env;
use std::fs;
use std::path::PathBuf;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
const LOCAL_CRATES_DIR: &str = "/mnt/crates/crates_file/";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} [local|mega]", args[0]);
        return;
    }

    match args[1].as_str() {
        "local" => import_from_local_repositories(),
        "mega" => import_from_mega().await,
        _ => println!("Invalid argument: expected 'local' or 'mega'"),
    }
}

/// FIXME: support extracting recursively
fn import_from_local_repositories() {
    println!("Importing from local repositories...");

    let repo_dir = LOCAL_CRATES_DIR;

    let mut programs: Vec<Program> = vec![];
    let mut libraries: Vec<Library> = vec![];
    let mut applications: Vec<Application> = vec![];
    let mut library_versions: Vec<LibraryVersion> = vec![];
    let mut application_versions: Vec<ApplicationVersion> = vec![];
    let mut versions: Vec<Version> = vec![];

    // traverse all the owner name dir in /mnt/crates/crates_file/
    for owner_entry in fs::read_dir(repo_dir).unwrap() {
        let owner_path = owner_entry.unwrap().path();
        if owner_path.is_dir() {
            //
            for repo_entry in fs::read_dir(&owner_path).unwrap() {
                let repo_path = repo_entry.unwrap().path();
                if repo_path.is_dir() {
                    if let Ok(repo) = Repository::open(&repo_path) {
                        // INFO: Start to Parse
                        println!("Processing repo: {}", repo_path.display());

                        //reset
                        hard_reset_to_head(&repo).unwrap();

                        let (program, uprogram) = extract_info_local(repo_path);
                        programs.push(program.clone());

                        let is_lib = match uprogram {
                            UProgram::Library(l) => {
                                libraries.push(l);
                                true
                            }
                            UProgram::Application(a) => {
                                applications.push(a);
                                false
                            }
                        };

                        // extract_dependencies
                        print_all_tags(&repo);

                        let uversions: Vec<UVersion> =
                            extract_all_tags(&repo, program.id, program.name, is_lib);

                        for v in uversions {
                            match v {
                                UVersion::LibraryVersion(l) => {
                                    library_versions.push(l.clone());
                                    versions.push(Version::new(&(l.name + &l.version)));
                                }
                                UVersion::ApplicationVersion(a) => {
                                    application_versions.push(a.clone());
                                    versions.push(Version::new(&(a.name + &a.version)));
                                }
                            }
                        }
                    } else {
                        println!("Not a git repo! {:?}", repo_path);
                    }
                }
            }
        }
    }

    // write into csv
    write_into_csv(PathBuf::from("./program.csv"), programs).unwrap();
    write_into_csv(PathBuf::from("./library.csv"), libraries).unwrap();
    write_into_csv(PathBuf::from("./application.csv"), applications).unwrap();
    write_into_csv(PathBuf::from("./library_version.csv"), library_versions).unwrap();
    write_into_csv(
        PathBuf::from("./application_version.csv"),
        application_versions,
    )
    .unwrap();
    write_into_csv(PathBuf::from("./version.csv"), versions).unwrap();
}

/// Import data from mega
/// It first clone the repositories locally from mega
async fn import_from_mega() {
    println!("Importing from MEGA...");
    let _ = clone_repos_from_pg(CLONE_CRATES_DIR).await;
    import_from_local_repositories();
}
