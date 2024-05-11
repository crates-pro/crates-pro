mod cli;
mod dep;
mod git;
mod info;
mod utils;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::info::extract_info_local;
use crate::info::write_into_csv;
use crate::{dep::parse_all_versions_of_a_repo, git::print_all_tags};
use cli::{Cli, Command};
use git::{clone_repos_from_pg, hard_reset_to_head};
use git2::Repository;
use log::*;
use model::crate_info::*;
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
const TUGRAPH_IMPORT_FILES: &str = "./tugraph_import_files/";

#[tokio::main]
async fn main() {
    let cli = Cli::from_args();

    dotenvy::dotenv().ok();
    pretty_env_logger::init();

    match cli.command {
        Command::Local => import_from_local_repositories(),
        Command::Mega => import_from_mega(&cli.mega_base).await,
    }
}

/// support extracting recursively
fn import_from_local_repositories() {
    info!("Importing from local repositories in {}", CLONE_CRATES_DIR);

    // structure in crates_info.rs
    let mut programs: Vec<Program> = vec![];
    let mut libraries: Vec<Library> = vec![];
    let mut applications: Vec<Application> = vec![];
    let mut library_versions: Vec<LibraryVersion> = vec![];
    let mut application_versions: Vec<ApplicationVersion> = vec![];
    let mut versions: Vec<Version> = vec![];

    // traverse all the owner name dir in /mnt/crates/local_crates_file/
    for owner_entry in fs::read_dir(CLONE_CRATES_DIR).unwrap() {
        let owner_path = owner_entry.unwrap().path();
        if owner_path.is_dir() {
            for repo_entry in fs::read_dir(&owner_path).unwrap() {
                let repo_path = repo_entry.unwrap().path();

                if repo_path.is_dir() {
                    if let Ok(repo) = Repository::open(&repo_path) {
                        // INFO: Start to Parse
                        trace!("Processing repo: {}", repo_path.display());
                        print_all_tags(&repo, false);

                        //reset, maybe useless
                        hard_reset_to_head(&repo).unwrap();

                        let pms = extract_info_local(repo_path);

                        for (program, uprogram) in pms {
                            programs.push(program.clone());

                            let _is_lib = match uprogram {
                                UProgram::Library(l) => {
                                    libraries.push(l);
                                    true
                                }
                                UProgram::Application(a) => {
                                    applications.push(a);
                                    false
                                }
                            };
                        }

                        let uversions: Vec<UVersion> = parse_all_versions_of_a_repo(&repo);
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

    let tugraph_import_files = PathBuf::from(TUGRAPH_IMPORT_FILES);

    fs::create_dir_all(tugraph_import_files.clone()).unwrap_or_else(|e| error!("Error: {}", e));

    // write into csv
    write_into_csv(tugraph_import_files.join("program.csv"), programs).unwrap();
    write_into_csv(tugraph_import_files.join("library.csv"), libraries).unwrap();
    write_into_csv(tugraph_import_files.join("application.csv"), applications).unwrap();
    write_into_csv(
        tugraph_import_files.join("library_version.csv"),
        library_versions,
    )
    .unwrap();
    write_into_csv(
        tugraph_import_files.join("application_version.csv"),
        application_versions,
    )
    .unwrap();
    write_into_csv(tugraph_import_files.join("version.csv"), versions).unwrap();
}

/// Import data from mega
/// It first clone the repositories locally from mega
async fn import_from_mega(mega_url_base: &str) {
    info!("Importing from MEGA...");
    let _ = clone_repos_from_pg(mega_url_base, CLONE_CRATES_DIR).await;
    import_from_local_repositories()
}
