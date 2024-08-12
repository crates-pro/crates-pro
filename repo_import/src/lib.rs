mod consumer;
mod git;
mod metadata_info;
mod utils;
mod version_info;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::consumer::KafkaConsumer;
use crate::metadata_info::extract_info_local;
use crate::utils::{get_program_by_name, name_join_version, write_into_csv};

use git::hard_reset_to_head;
use git2::Repository;
use log::*;
use model::{repo_sync_model, tugraph_model::*};
use rdkafka::Message;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use version_info::VersionUpdater;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
const TUGRAPH_IMPORT_FILES_PG: &str = "./tugraph_import_files_mq/";

pub use utils::reset_mq;

pub struct ImportDriver {
    context: ImportContext,
    consumer: KafkaConsumer,
}

impl ImportDriver {
    pub async fn new(dont_clone: bool) -> Self {
        info!("Setup Importing from MQ...");
        let broker = env::var("KAFKA_BROKER").unwrap();
        let topic = env::var("KAFKA_TOPIC").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();

        tracing::info!("{},{},{}", broker, topic, group_id);

        let context = ImportContext {
            dont_clone,
            ..Default::default()
        };

        let consumer = KafkaConsumer::new(&broker, &group_id, &[&topic]);

        Self { context, consumer }
    }

    pub async fn import_from_mq_for_a_message(&mut self) {
        let git_url_base = env::var("MEGA_BASE_URL").unwrap();
        let message = match self.consumer.consume_once().await {
            None => return,
            Some(m) => m,
        };

        let model =
            match serde_json::from_slice::<repo_sync_model::Model>(message.payload().unwrap()) {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!("Error while deserializing message payload: {:?}", e);
                    None
                }
            };
        tracing::info!(
            "key: '{:?}', payload: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
            message.key(),
            model,
            message.topic(),
            message.partition(),
            message.offset(),
            message.timestamp()
        );

        let mega_url_suffix = model.unwrap().mega_url;

        let local_repo_path = self
            .context
            .clone_a_repo_by_url(CLONE_CRATES_DIR, &git_url_base, &mega_url_suffix)
            .await
            .unwrap_or_else(|_| panic!("Failed to clone repo {}", mega_url_suffix));

        self.context
            .parse_a_local_repo(local_repo_path, mega_url_suffix)
            .await
            .unwrap();

        self.context.write_tugraph_import_files();
    }
}

#[derive(Debug, Default)]
pub struct ImportContext {
    pub dont_clone: bool,

    // data to write into
    /// vertex
    pub programs: Vec<Program>,

    pub libraries: Vec<Library>,
    pub applications: Vec<Application>,
    pub library_versions: Vec<LibraryVersion>,
    pub application_versions: Vec<ApplicationVersion>,
    pub versions: Vec<Version>,

    /// edge
    has_lib_type: Vec<HasType>,
    has_app_type: Vec<HasType>,

    lib_has_version: Vec<HasVersion>,
    app_has_version: Vec<HasVersion>,

    lib_has_dep_version: Vec<HasDepVersion>,
    app_has_dep_version: Vec<HasDepVersion>,

    depends_on: Vec<DependsOn>,

    /// help is judge whether it is a new program
    program_memory: HashSet<model::general_model::Program>,
    /// help us judge whether it is a new version
    version_memory: HashSet<model::general_model::Version>,

    version_updater: VersionUpdater,
}

impl ImportContext {
    /// Import data from mega
    /// It first clone the repositories locally from mega

    async fn parse_a_local_repo(
        &mut self,
        repo_path: PathBuf,
        mega_url: String,
    ) -> Result<(), String> {
        if repo_path.is_dir() && Path::new(&repo_path).join(".git").is_dir() {
            match Repository::open(&repo_path) {
                Err(e) => {
                    tracing::error!("Not a git repo: {:?}, Err: {}", repo_path, e);
                }
                Ok(_) => {
                    // It'a a valid git repository. Start to parse it.
                    tracing::debug!("Processing repo: {}", repo_path.display());

                    //reset, maybe useless
                    hard_reset_to_head(&repo_path)
                        .await
                        .map_err(|x| format!("{:?}", x))?;

                    let all_programs = self
                        .collect_and_filter_programs(&repo_path, &mega_url)
                        .await;

                    let all_dependencies = self.collect_and_filter_versions(&repo_path).await;

                    for (program, has_type, uprogram) in all_programs {
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

                        // NOTE: memorize program
                        self.program_memory
                            .insert(model::general_model::Program::new(
                                &program.name,
                                &program.mega_url.clone().unwrap(),
                            ));
                    }

                    for dependencies in all_dependencies {
                        let name = dependencies.crate_name.clone();
                        let version = dependencies.version.clone();

                        // check whether the crate version exists.
                        let (program, uprogram) = match get_program_by_name(&name) {
                            Some((program, uprogram)) => (program, uprogram),
                            None => {
                                // continue, dont parse
                                continue;
                            }
                        };

                        self.version_updater.update_depends_on(&dependencies).await;

                        let has_version = HasVersion {
                            SRC_ID: program.id.clone(),
                            DST_ID: name_join_version(&name, &version), //FIXME: version id undecided
                        };

                        let dep_version = Version {
                            name_and_version: name_join_version(&name, &version),
                        };

                        #[allow(non_snake_case)]
                        let SRC_ID = name_join_version(&name, &version);
                        #[allow(non_snake_case)]
                        let DST_ID = name_join_version(&name, &version);
                        let has_dep_version = HasDepVersion { SRC_ID, DST_ID };

                        let islib = uprogram.index() == 0;
                        if islib {
                            let version = LibraryVersion::new(
                                program.id.clone(),
                                &name.clone(),
                                &version.clone(),
                                "???",
                            );

                            self.library_versions.push(version);
                            self.lib_has_version.push(has_version);
                            self.lib_has_dep_version.push(has_dep_version);
                        } else {
                            let version = ApplicationVersion::new(
                                program.id.clone(),
                                name.clone(),
                                version.clone(),
                            );

                            self.application_versions.push(version.clone());
                            self.app_has_version.push(has_version);
                            self.app_has_dep_version.push(has_dep_version);
                        }
                        self.versions.push(dep_version);

                        self.depends_on
                            .append(&mut self.version_updater.to_depends_on_edges().await);

                        // NOTE: memorize version, insert the new version into memory
                        self.version_memory
                            .insert(model::general_model::Version::new(
                                &dependencies.crate_name,
                                &dependencies.version,
                            ));
                    }

                    tracing::trace!("Finish processing repo: {}", repo_path.display());
                }
            }
        } else {
            tracing::error!("{} is not a directory", repo_path.display());
        }
        Ok(())
    }

    async fn collect_and_filter_programs(
        &self,
        repo_path: &Path,
        mega_url: &str,
    ) -> Vec<(Program, HasType, UProgram)> {
        let all_programs: Vec<(Program, HasType, UProgram)> =
            extract_info_local(repo_path.to_path_buf(), mega_url.to_owned())
                .await
                .into_iter()
                .filter(|(p, _, _)| {
                    !self
                        .program_memory
                        .contains(&model::general_model::Program::new(
                            &p.name,
                            &p.mega_url.clone().unwrap(),
                        ))
                })
                .collect();
        all_programs
    }
    async fn collect_and_filter_versions(
        &self,
        repo_path: &PathBuf,
    ) -> Vec<version_info::Dependencies> {
        // get all versions and dependencies
        // filter out new versions!!!
        let all_dependencies: Vec<version_info::Dependencies> = self
            .parse_all_versions_of_a_repo(repo_path)
            .await
            .into_iter()
            .filter(|x| {
                !self
                    .version_memory
                    .contains(&model::general_model::Version::new(
                        &x.crate_name,
                        &x.version,
                    ))
            })
            .collect();
        all_dependencies
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
