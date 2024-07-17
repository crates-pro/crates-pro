mod consumer;
mod git;
mod metadata_info;
mod utils;
mod version_info;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::metadata_info::extract_info_local;
use crate::utils::write_into_csv;
use crates_sync::consumer::consume;
use crates_sync::{consumer::MessageCallback, repo_sync_model};
use futures::future::BoxFuture;
use futures::FutureExt;
use git::hard_reset_to_head;
use git2::Repository;
use log::*;
use model::tugraph_model::*;
use rdkafka::{message::BorrowedMessage, Message};
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, sync::Arc};
use std::{thread::sleep, time::Duration};
use tokio::sync::Mutex;
use version_info::VersionUpdater;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
const TUGRAPH_IMPORT_FILES_PG: &str = "./tugraph_import_files_mq/";

pub async fn repo_main(dont_clone: bool, git_url_base: &str) {
    //driver.import_from_mega(&cli.mega_base).await,
    let import_driver = ImportDriver {
        dont_clone,
        ..ImportDriver::default()
    };
    let _ = utils::reset_mq().await;
    ImportDriver::import_from_mq(Arc::new(Mutex::new(import_driver)), git_url_base).await;
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
    async fn import_from_mq(driver: Arc<Mutex<Self>>, git_url_base: &str) {
        info!("Importing from MQ...");
        let broker = env::var("KAFKA_BROKER").unwrap();
        let topic = env::var("KAFKA_TOPIC").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();
        tracing::info!("{},{},{}", broker, topic, group_id);

        loop {
            let new_message_entry = Arc::new(Mutex::new(RepoSyncCallback {
                git_url_base: git_url_base.to_owned(),
                driver: driver.clone(),
            }));
            consume(&broker, &group_id, &[&topic], new_message_entry.clone()).await;

            println!("{:?}", *new_message_entry);
        }
    }

    async fn parse_a_local_repo(&mut self, repo_path: PathBuf) -> Result<(), String> {
        if repo_path.is_dir() && Path::new(&repo_path).join(".git").is_dir() {
            if let Ok(_repo) = Repository::open(&repo_path) {
                // INFO: Start to Parse a git repository
                tracing::trace!("");
                tracing::trace!("Processing repo: {}", repo_path.display());

                //reset, maybe useless
                hard_reset_to_head(&repo_path)
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

                let (uversions, depends_on) = self.parse_all_versions_of_a_repo(&repo_path).await;
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

#[derive(Debug)]
pub struct RepoSyncCallback {
    git_url_base: String,
    driver: Arc<Mutex<ImportDriver>>,
}

impl MessageCallback for RepoSyncCallback {
    fn on_message<'a>(&'a mut self, m: &'a BorrowedMessage<'a>) -> BoxFuture<'a, ()> {
        async move {
            let model = match serde_json::from_slice::<repo_sync_model::Model>(m.payload().unwrap())
            {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("Error while deserializing message payload: {:?}", e);
                    None
                }
            };
            tracing::info!(
            "key: '{:?}', payload: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
            m.key(),
            model,
            m.topic(),
            m.partition(),
            m.offset(),
            m.timestamp()
        );

            let mega_url_suffix = &model.unwrap().mega_url;

            let local_repo_path = self
                .driver
                .lock()
                .await
                .clone_a_repo_by_url(CLONE_CRATES_DIR, &self.git_url_base, mega_url_suffix)
                .await
                .unwrap_or_else(|_| panic!("Failed to clone repo {}", mega_url_suffix));

            self.driver
                .lock()
                .await
                .parse_a_local_repo(local_repo_path)
                .await
                .unwrap();

            self.driver.lock().await.write_tugraph_import_files();

            //sleep(Duration::from_millis(2000));
        }
        .boxed()
    }
}
