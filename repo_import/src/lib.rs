mod consumer;
mod git;
mod metadata_info;
mod utils;
mod version_info;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::consumer::RepoSyncCallback;
//use crate::git::print_all_tags;
use crate::metadata_info::extract_info_local;
use crate::utils::write_into_csv;

use crates_sync::consumer::consume;
use git::hard_reset_to_head;
use git2::Repository;
use log::*;
use model::tugraph_model::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use version_info::VersionUpdater;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
const TUGRAPH_IMPORT_FILES_PG: &str = "./tugraph_import_files_pg/";

pub async fn repo_main(dont_clone: bool, git_url_base: &str) {
    //driver.import_from_mega(&cli.mega_base).await,
    let mut import_driver = ImportDriver {
        dont_clone,
        ..ImportDriver::default()
    };
    import_driver.import_from_mq(git_url_base).await;
}

#[derive(Debug, Default)]
struct ImportDriver {
    dont_clone: bool,

    // data to write into
    /// vertex
    programs: Vec<Program>,

    libraries: Vec<Library>,
    applications: Vec<Application>,
    library_versions: Vec<LibraryVersion>,
    application_versions: Vec<ApplicationVersion>,
    versions: Vec<Version>,

    /// edge
    has_lib_type: Vec<HasType>,
    has_app_type: Vec<HasType>,

    lib_has_version: Vec<HasVersion>,
    app_has_version: Vec<HasVersion>,

    lib_has_dep_version: Vec<HasDepVersion>,
    app_has_dep_version: Vec<HasDepVersion>,

    depends_on: Vec<DependsOn>,

    version_updater: VersionUpdater,
}

impl ImportDriver {
    /// Import data from mega
    /// It first clone the repositories locally from mega
    async fn import_from_mq(&mut self, mega_url_base: &str) {
        info!("Importing from MQ...");
        let broker = env::var("KAFKA_BROKER").unwrap();
        let topic = env::var("KAFKA_TOPIC").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();
        tracing::info!("{},{},{}", broker, topic, group_id);

        loop {
            let new_message_entry = Arc::new(Mutex::new(RepoSyncCallback::default()));
            consume(&broker, &group_id, &[&topic], new_message_entry.clone()).await;

            let mega_url_suffix = &{
                let inner_entry = new_message_entry.lock().await.entry.clone();
                assert!(inner_entry.is_some());
                inner_entry.unwrap().mega_url
            };

            let local_repo_path = self
                .clone_a_repo_by_url(CLONE_CRATES_DIR, mega_url_base, mega_url_suffix)
                .await
                .unwrap_or_else(|_| panic!("Failed to clone repo {}", mega_url_suffix));

            self.parse_a_local_repo(local_repo_path).await.unwrap();

            self.write_tugraph_import_files();
            println!("{:?}", *new_message_entry);
        }
    }

    async fn parse_a_local_repo(&mut self, repo_path: PathBuf) -> Result<(), String> {
        if repo_path.is_dir() && Path::new(&repo_path).join(".git").is_dir() {
            if let Ok(repo) = Repository::open(&repo_path) {
                // INFO: Start to Parse a git repository
                tracing::trace!("");
                tracing::trace!("Processing repo: {}", repo_path.display());

                //reset, maybe useless
                hard_reset_to_head(&repo)
                    .await
                    .map_err(|x| format!("{:?}", x))?;

                let pms = extract_info_local(repo_path.clone());
                //println!("{:?}", pms);

                for (program, has_type, uprogram) in pms {
                    self.programs.push(program.clone());

                    match uprogram {
                        UProgram::Library(l) => {
                            self.libraries.push(l);
                            self.has_lib_type.push(has_type.clone());
                        }
                        UProgram::Application(a) => {
                            self.applications.push(a);
                            self.has_app_type.push(has_type.clone());
                        }
                    };
                }

                let (uversions, depends_on) = self.parse_all_versions_of_a_repo(&repo).await;
                for (has_version, uv, v, has_dep) in uversions {
                    match uv {
                        UVersion::LibraryVersion(l) => {
                            self.library_versions.push(l.clone());
                            self.lib_has_version.push(has_version);
                            self.lib_has_dep_version.push(has_dep);
                        }
                        UVersion::ApplicationVersion(a) => {
                            self.application_versions.push(a.clone());
                            self.app_has_version.push(has_version);
                            self.app_has_dep_version.push(has_dep);
                        }
                    }

                    self.versions.push(v);
                }
                for dep_on in depends_on {
                    self.depends_on.push(dep_on);
                }

                tracing::trace!("Finish processing repo: {}", repo_path.display());
            } else {
                tracing::error!("Not a git repo! {:?}", repo_path);
            }
        } else {
            tracing::error!("{} is not a directory", repo_path.display());
        }
        Ok(())
    }

    /// write data base into tugraph import files
    fn write_tugraph_import_files(&self) {
        let tugraph_import_files = PathBuf::from(TUGRAPH_IMPORT_FILES_PG);

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

        // edge
        let _ = write_into_csv(
            tugraph_import_files.join("has_lib_type.csv"),
            self.has_lib_type.clone(),
        );
        let _ = write_into_csv(
            tugraph_import_files.join("has_app_type.csv"),
            self.has_app_type.clone(),
        );
        let _ = write_into_csv(
            tugraph_import_files.join("lib_has_version.csv"),
            self.lib_has_version.clone(),
        );
        let _ = write_into_csv(
            tugraph_import_files.join("app_has_version.csv"),
            self.app_has_version.clone(),
        );

        let _ = write_into_csv(
            tugraph_import_files.join("lib_has_dep_version.csv"),
            self.lib_has_dep_version.clone(),
        );
        let _ = write_into_csv(
            tugraph_import_files.join("app_has_dep_version.csv"),
            self.app_has_dep_version.clone(),
        );
        let _ = write_into_csv(
            tugraph_import_files.join("depends_on.csv"),
            self.depends_on.clone(),
        );
    }
}
