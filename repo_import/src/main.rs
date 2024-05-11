mod cli;
mod git;
mod metadata_info;
mod utils;
mod version_info;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::metadata_info::extract_info_local;
use crate::metadata_info::write_into_csv;
use crate::{git::print_all_tags, version_info::parse_all_versions_of_a_repo};
use cli::{Cli, Command};
use git::hard_reset_to_head;
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

    let mut driver = ImportDriver::default();

    match cli.command {
        Command::Mega => driver.import_from_mega(&cli.mega_base).await,
    }
}

#[derive(Debug, Default)]
struct ImportDriver {
    // data to write into
    programs: Vec<Program>,
    libraries: Vec<Library>,
    applications: Vec<Application>,
    library_versions: Vec<LibraryVersion>,
    application_versions: Vec<ApplicationVersion>,
    versions: Vec<Version>,
}

impl ImportDriver {
    /// Import data from mega
    /// It first clone the repositories locally from mega
    async fn import_from_mega(&mut self, mega_url_base: &str) {
        info!("Importing from MEGA...");
        let _ = self
            .clone_repos_from_pg(mega_url_base, CLONE_CRATES_DIR)
            .await;
        self.parse_local_repositories()
    }

    /// support extracting recursively
    fn parse_local_repositories(&mut self) {
        // traverse all the owner name dir in /mnt/crates/local_crates_file/
        for owner_entry in fs::read_dir(CLONE_CRATES_DIR).unwrap() {
            let owner_path = owner_entry.unwrap().path();
            if owner_path.is_dir() {
                for repo_entry in fs::read_dir(&owner_path).unwrap() {
                    let repo_path = repo_entry.unwrap().path();

                    if repo_path.is_dir() {
                        if let Ok(repo) = Repository::open(&repo_path) {
                            // INFO: Start to Parse a git repository
                            trace!("");
                            trace!("Processing repo: {}", repo_path.display());

                            print_all_tags(&repo, false);

                            //reset, maybe useless
                            hard_reset_to_head(&repo).unwrap();

                            let pms = extract_info_local(repo_path);
                            println!("{:?}", pms);

                            for (program, uprogram) in pms {
                                self.programs.push(program.clone());

                                let _is_lib = match uprogram {
                                    UProgram::Library(l) => {
                                        self.libraries.push(l);
                                        true
                                    }
                                    UProgram::Application(a) => {
                                        self.applications.push(a);
                                        false
                                    }
                                };
                            }

                            let uversions: Vec<UVersion> = parse_all_versions_of_a_repo(&repo);
                            for v in uversions {
                                match v {
                                    UVersion::LibraryVersion(l) => {
                                        self.library_versions.push(l.clone());
                                        self.versions.push(Version::new(&(l.name + &l.version)));
                                    }
                                    UVersion::ApplicationVersion(a) => {
                                        self.application_versions.push(a.clone());
                                        self.versions.push(Version::new(&(a.name + &a.version)));
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

        self.write_tugraph_import_files();
    }

    /// write data base into tugraph import files
    fn write_tugraph_import_files(&self) {
        let tugraph_import_files = PathBuf::from(TUGRAPH_IMPORT_FILES);

        fs::create_dir_all(tugraph_import_files.clone()).unwrap_or_else(|e| error!("Error: {}", e));

        // write into csv
        write_into_csv(
            tugraph_import_files.join("program.csv"),
            self.programs.clone(),
        )
        .unwrap();
        write_into_csv(
            tugraph_import_files.join("library.csv"),
            self.libraries.clone(),
        )
        .unwrap();
        write_into_csv(
            tugraph_import_files.join("application.csv"),
            self.applications.clone(),
        )
        .unwrap();
        write_into_csv(
            tugraph_import_files.join("library_version.csv"),
            self.library_versions.clone(),
        )
        .unwrap();
        write_into_csv(
            tugraph_import_files.join("application_version.csv"),
            self.application_versions.clone(),
        )
        .unwrap();
        write_into_csv(
            tugraph_import_files.join("version.csv"),
            self.versions.clone(),
        )
        .unwrap();
    }
}
