mod crate_info;
mod git;
mod kafka_handler;
mod utils;
mod version_info;

extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate lazy_static;

use crate::crate_info::extract_info_local;
use crate::kafka_handler::KafkaHandler;
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
// const TUGRAPH_IMPORT_FILES_PG: &str = "./tugraph_import_files_mq/";

pub use kafka_handler::reset_kafka_offset;

pub struct ImportDriver {
    context: ImportContext,
    handler: KafkaHandler,
}

impl ImportDriver {
    pub async fn new(dont_clone: bool) -> Self {
        info!("Start to setup Kafka client.");
        let broker = env::var("KAFKA_BROKER").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();

        tracing::info!("Kafka parameters: {},{}", broker, group_id);

        let context = ImportContext {
            dont_clone,
            ..Default::default()
        };

        let handler =
            KafkaHandler::new(&broker, &group_id, &env::var("KAFKA_IMPORT_TOPIC").unwrap());

        info!("Finish to setup Kafka client.");

        Self { context, handler }
    }

    pub async fn import_from_mq_for_a_message(&mut self) -> Result<(), ()> {
        tracing::info!("Start to import from a message!");
        // //tracing::debug
        // println!("Context size: {}", self.context.calculate_memory_usage());
        // let kafka_import_topic = env::var("KAFKA_IMPORT_TOPIC").unwrap();
        let kafka_analysis_topic = env::var("KAFKA_ANALYSIS_TOPIC").unwrap();
        let git_url_base = env::var("MEGA_BASE_URL").unwrap();
        let message = match self.handler.consume_once().await {
            None => {
                tracing::warn!("No message in Kafka, please check it!");
                return Err(());
            }
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
            "Received a message, key: '{:?}', payload: '{:?}', topic: {}, partition: {}, offset: {}, timestamp: {:?}",
            message.key(),
            model,
            message.topic(),
            message.partition(),
            message.offset(),
            message.timestamp()
        );

        let mega_url_suffix = model.unwrap().mega_url;

        let clone_crates_dir =
            env::var("CLONE_CRATES_DIR").unwrap_or_else(|_| CLONE_CRATES_DIR.to_string());

        let local_repo_path = match self
            .context
            .clone_a_repo_by_url(&clone_crates_dir, &git_url_base, &mega_url_suffix)
            .await
        {
            Ok(x) => x,
            _ => {
                tracing::error!("Failed to clone repo {}", mega_url_suffix);
                return Err(());
            }
        };

        let new_versions = self
            .context
            .parse_a_local_repo_and_return_new_versions(local_repo_path, mega_url_suffix)
            .await
            .unwrap();

        for ver in new_versions {
            self.handler.send_message(
                &kafka_analysis_topic,
                "",
                &serde_json::to_string(&ver).unwrap(),
            );
        }

        //self.context.write_tugraph_import_files();

        tracing::info!("Finish to import from a message!");
        Ok(())
    }
}

/// internal structure,
/// a context for repo parsing and importing.
#[derive(Debug, Default)]
struct ImportContext {
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
    async fn parse_a_local_repo_and_return_new_versions(
        &mut self,
        repo_path: PathBuf,
        git_url: String,
    ) -> Result<Vec<model::general_model::VersionWithTag>, String> {
        let mut new_versions = vec![];

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

                    let all_programs = self.collect_and_filter_programs(&repo_path, &git_url).await;

                    let all_dependencies =
                        self.collect_and_filter_versions(&repo_path, &git_url).await;

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
                        let git_url = dependencies.git_url.clone();
                        let tag_name = dependencies.tag_name.clone();

                        // reserve for kafka sending
                        new_versions.push(model::general_model::VersionWithTag::new(
                            &name, &version, &git_url, &tag_name,
                        ));

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
                            .clone_from(&(self.version_updater.to_depends_on_edges().await));

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
        Ok(new_versions)
    }

    async fn collect_and_filter_programs(
        &self,
        repo_path: &Path,
        git_url: &str,
    ) -> Vec<(Program, HasType, UProgram)> {
        tracing::info!("Start to collect_and_filter_programs {:?}", repo_path);
        let all_programs: Vec<(Program, HasType, UProgram)> =
            extract_info_local(repo_path.to_path_buf(), git_url.to_owned())
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
        tracing::info!("Finish to collect_and_filter_programs {:?}", repo_path);
        all_programs
    }
    async fn collect_and_filter_versions(
        &self,
        repo_path: &PathBuf,
        git_url: &str,
    ) -> Vec<version_info::Dependencies> {
        tracing::info!("Start to collect_and_filter_versions {:?}", repo_path);
        // get all versions and dependencies
        // filter out new versions!!!
        let all_dependencies: Vec<version_info::Dependencies> = self
            .parse_all_versions_of_a_repo(repo_path, git_url)
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
            //.filter(|x| semver::Version::parse(&x.version).is_ok())
            .collect();

        tracing::info!("Finish to collect_and_filter_versions {:?}", repo_path);
        all_dependencies
    }

    /// write data base into tugraph import files
    fn write_tugraph_import_files(&self) {
        tracing::info!("Start to write");
        let tugraph_import_files = PathBuf::from(env::var("TUGRAPH_IMPORT_FILES_PG").unwrap());
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
        tracing::info!("Finish to write");
    }
}

impl ImportContext {
    #[allow(unused)]
    fn calculate_memory_usage(&self) -> String {
        use std::fmt::Write;
        use std::mem;
        let mut output = String::new();
        let bytes_per_gb = 1_073_741_824; // 1024^3

        let fields = [
            (
                "Programs",
                self.programs.capacity(),
                mem::size_of::<Program>(),
            ),
            (
                "Libraries",
                self.libraries.capacity(),
                mem::size_of::<Library>(),
            ),
            (
                "Applications",
                self.applications.capacity(),
                mem::size_of::<Application>(),
            ),
            (
                "LibraryVersions",
                self.library_versions.capacity(),
                mem::size_of::<LibraryVersion>(),
            ),
            (
                "ApplicationVersions",
                self.application_versions.capacity(),
                mem::size_of::<ApplicationVersion>(),
            ),
            (
                "Versions",
                self.versions.capacity(),
                mem::size_of::<Version>(),
            ),
            (
                "HasLibType",
                self.has_lib_type.capacity(),
                mem::size_of::<HasType>(),
            ),
            (
                "HasAppType",
                self.has_app_type.capacity(),
                mem::size_of::<HasType>(),
            ),
            (
                "LibHasVersion",
                self.lib_has_version.capacity(),
                mem::size_of::<HasVersion>(),
            ),
            (
                "AppHasVersion",
                self.app_has_version.capacity(),
                mem::size_of::<HasVersion>(),
            ),
            (
                "LibHasDepVersion",
                self.lib_has_dep_version.capacity(),
                mem::size_of::<HasDepVersion>(),
            ),
            (
                "AppHasDepVersion",
                self.app_has_dep_version.capacity(),
                mem::size_of::<HasDepVersion>(),
            ),
            (
                "DependsOn",
                self.depends_on.capacity(),
                mem::size_of::<DependsOn>(),
            ),
            (
                "ProgramMemory",
                self.program_memory.capacity(),
                mem::size_of::<model::general_model::Program>(),
            ),
            (
                "VersionMemory",
                self.version_memory.capacity(),
                mem::size_of::<model::general_model::Version>(),
            ),
        ];

        for (name, capacity, size) in &fields {
            let total_size_bytes = capacity * size;
            let total_size_gb = total_size_bytes as f64 / bytes_per_gb as f64;
            write!(output, " [{}: {:.4} GB]", name, total_size_gb).unwrap();
        }

        output
    }
}
