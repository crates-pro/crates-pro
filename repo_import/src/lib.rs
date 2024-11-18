mod crate_info;
mod git;
mod kafka_handler;
mod utils;
mod version_info;

extern crate lazy_static;
extern crate pretty_env_logger;

use crate::crate_info::extract_info_local;
use crate::kafka_handler::KafkaHandler;
use crate::utils::{
    extract_namespace, get_program_by_name, insert_namespace_by_repo_path, name_join_version,
    write_into_csv,
};
use git::hard_reset_to_head;
use git2::Repository;
use model::{repo_sync_model, tugraph_model::*};
use rdkafka::error::KafkaError;
use rdkafka::message::BorrowedMessage;
use rdkafka::Message;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use url::Url;
use version_info::VersionUpdater;

const CLONE_CRATES_DIR: &str = "/mnt/crates/local_crates_file/";
// const TUGRAPH_IMPORT_FILES_PG: &str = "./tugraph_import_files_mq/";

pub use kafka_handler::reset_kafka_offset;

pub enum MessageKind {
    Mega,
    UserUpload,
}

pub struct ImportMessage<'a> {
    kind: MessageKind,
    message: BorrowedMessage<'a>,
}

pub struct ImportDriver {
    pub context: ImportContext,
    pub import_handler: KafkaHandler,
    pub user_import_handler: KafkaHandler,
    pub sender_handler: KafkaHandler,
}

impl ImportDriver {
    pub async fn new(dont_clone: bool) -> Self {
        tracing::info!("Start to setup Kafka client.");
        let broker = env::var("KAFKA_BROKER").unwrap();
        let group_id = env::var("KAFKA_GROUP_ID").unwrap();

        tracing::info!("Kafka parameters: {},{}", broker, group_id);

        let context = ImportContext {
            dont_clone,
            ..Default::default()
        };

        // Data from Mega
        let import_handler = KafkaHandler::new_consumer(
            &broker,
            &group_id,
            &env::var("KAFKA_IMPORT_TOPIC").unwrap_or("REPO_SYNC_STATUS.dev.0902".to_owned()),
        )
        .expect("Invalid import kafka handler");

        // Data from user-uploading
        let user_import_handler = KafkaHandler::new_consumer(
            &broker,
            &group_id,
            &env::var("KAFKA_USER_IMPORT_TOPIC").unwrap_or("USER_IMPORT".to_owned()),
        )
        .expect("Invalid import kafka handler");

        // sending for analysis
        let sender_handler =
            KafkaHandler::new_producer(&broker).expect("Invalid import kafka handler");

        tracing::info!("Finish to setup Kafka client.");

        Self {
            context,
            import_handler,
            user_import_handler,
            sender_handler,
        }
    }

    async fn consume_message(&self) -> Result<ImportMessage, KafkaError> {
        // try to get data from user_import_handler
        if let Ok(message) = self.user_import_handler.consume_once().await {
            tracing::info!("Receive a user upload message!");
            return Ok(ImportMessage {
                kind: MessageKind::UserUpload,
                message,
            });
        }
        // if there is no data from user_import_handler，try to fetch data from import_handler
        else if let Ok(message) = self.import_handler.consume_once().await {
            return Ok(ImportMessage {
                kind: MessageKind::Mega,
                message,
            });
        };

        Err(KafkaError::NoMessageReceived)
    }

    pub async fn import_from_mq_for_a_message(&mut self) -> Result<(), ()> {
        tracing::info!("Start to import from a message!");
        // //tracing::debug
        // println!("Context size: {}", self.context.calculate_memory_usage());
        // let kafka_import_topic = env::var("KAFKA_IMPORT_TOPIC").unwrap();
        let kafka_analysis_topic = env::var("KAFKA_ANALYSIS_TOPIC").unwrap();
        let git_url_base = env::var("MEGA_BASE_URL").unwrap();

        let ImportMessage { kind, message } = match self.consume_message().await {
            Err(_) => {
                tracing::warn!("No message in Kafka, please check it!");
                return Err(());
            }
            Ok(m) => m,
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
        if matches!(kind, MessageKind::UserUpload) {
            //from user upload
            tracing::info!("user upload path:{}", model.clone().unwrap().mega_url);
            let usr_upload_path = model.unwrap().mega_url;
            let namespace = extract_namespace(&usr_upload_path).expect("Failed to parse URL");
            let path = PathBuf::from(&usr_upload_path);
            insert_namespace_by_repo_path(path.to_str().unwrap().to_string(), namespace.clone());
            let new_versions = self
                .context
                .parse_a_local_repo_and_return_new_versions(path, "".to_string())
                .await
                .unwrap();
            for ver in new_versions {
                self.sender_handler
                    .send_message(
                        &kafka_analysis_topic,
                        "",
                        &serde_json::to_string(&ver).unwrap(),
                    )
                    .await;
            }
        } else {
            //from mega
            let mega_url_suffix = model.unwrap().mega_url;

            let clone_crates_dir =
                env::var("NEW_CRATES_DIR").unwrap_or_else(|_| CLONE_CRATES_DIR.to_string());
            //changes clone_or_not_clone
            let git_url = {
                let git_url_base = Url::parse(&git_url_base)
                    .unwrap_or_else(|_| panic!("Failed to parse mega url base: {}", &git_url_base));
                git_url_base
                    .join(&mega_url_suffix)
                    .expect("Failed to join url path")
            };
            let namespace = extract_namespace(git_url.as_ref()).expect("Failed to parse URL");
            let path = PathBuf::from(&clone_crates_dir).join(namespace.clone());

            //changes
            if !path.is_dir() {
                //if user_upload, no clone
                tracing::info!("dir {} not exist", path.to_str().unwrap().to_string());
                let clone_start_time = Instant::now();
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
                let clone_need_time = clone_start_time.elapsed();
                tracing::info!("clone need time: {:?}", clone_need_time);
                let new_versions = self
                    .context
                    .parse_a_local_repo_and_return_new_versions(local_repo_path, mega_url_suffix)
                    .await
                    .unwrap();

                if matches!(kind, MessageKind::UserUpload) {
                    for ver in new_versions {
                        self.sender_handler
                            .send_message(
                                &kafka_analysis_topic,
                                "",
                                &serde_json::to_string(&ver).unwrap(),
                            )
                            .await;
                    }
                }
            } else {
                tracing::info!("dir {} already exist", path.to_str().unwrap().to_string());
                let insert_time = Instant::now();
                insert_namespace_by_repo_path(
                    path.to_str().unwrap().to_string(),
                    namespace.clone(),
                );
                let insert_need_time = insert_time.elapsed();
                tracing::info!(
                    "insert_namespace_by_repo_path need time: {:?}",
                    insert_need_time
                );
                let new_versions = self
                    .context
                    .parse_a_local_repo_and_return_new_versions(path, mega_url_suffix)
                    .await
                    .unwrap();

                if matches!(kind, MessageKind::UserUpload) {
                    for ver in new_versions {
                        self.sender_handler
                            .send_message(
                                &kafka_analysis_topic,
                                "",
                                &serde_json::to_string(&ver).unwrap(),
                            )
                            .await;
                    }
                }
            } //changes
        }
        //self.context.write_tugraph_import_files();

        tracing::info!("Finish to import from a message!");
        Ok(())
    }
}

/// internal structure,
/// a context for repo parsing and importing.
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

    pub depends_on: Vec<DependsOn>,

    /// help is judge whether it is a new program
    program_memory: HashSet<model::general_model::Program>,
    /// help us judge whether it is a new version
    version_memory: HashSet<model::general_model::Version>,

    pub version_updater: VersionUpdater,
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
                    tracing::info!("Processing repo: {}", repo_path.display());

                    //reset, maybe useless
                    let hard_reset_time = Instant::now();
                    hard_reset_to_head(&repo_path)
                        .await
                        .map_err(|x| format!("{:?}", x))?;
                    let hard_reset_need_time = hard_reset_time.elapsed();
                    tracing::info!("hard_reset_to_head need time: {:?}", hard_reset_need_time);
                    let all_programs = self.collect_and_filter_programs(&repo_path, &git_url).await;

                    let all_dependencies =
                        self.collect_and_filter_versions(&repo_path, &git_url).await;
                    let proccess_time = Instant::now();

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

                    //let depend_time = Instant::now();
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

                        //self.depends_on
                        //    .clone_from(&(self.version_updater.to_depends_on_edges().await));

                        // NOTE: memorize version, insert the new version into memory
                        self.version_memory
                            .insert(model::general_model::Version::new(
                                &dependencies.crate_name,
                                &dependencies.version,
                            ));
                    }
                    let proccess_need_time = proccess_time.elapsed();
                    tracing::info!("Finish processing repo: {}", repo_path.display());
                    tracing::info!("processing repo need time: {:?}", proccess_need_time);
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
        let collect_time = Instant::now();
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
        let collect_need_time = collect_time.elapsed();
        tracing::info!("Finish to collect_and_filter_programs {:?}", repo_path);
        tracing::info!(
            "collect_and_filter_programs need time: {:?}",
            collect_need_time
        );
        all_programs
    }
    async fn collect_and_filter_versions(
        &self,
        repo_path: &PathBuf,
        git_url: &str,
    ) -> Vec<version_info::Dependencies> {
        tracing::info!("Start to collect_and_filter_versions {:?}", repo_path);
        let collect_time = Instant::now();
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
        let collect_need_time = collect_time.elapsed();
        tracing::info!("Finish to collect_and_filter_versions {:?}", repo_path);
        tracing::info!(
            "collect_and_filter_versions need time: {:?}",
            collect_need_time
        );
        all_dependencies
    }

    /// write data base into tugraph import files
    pub fn write_tugraph_import_files(&self) {
        tracing::info!("Start to write");
        let write_time = Instant::now();
        let tugraph_import_files = PathBuf::from(env::var("TUGRAPH_IMPORT_FILES_PG").unwrap());
        fs::create_dir_all(tugraph_import_files.clone())
            .unwrap_or_else(|e| tracing::error!("Error: {}", e));

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
        let write_need_time = write_time.elapsed();
        tracing::info!("write need time: {:?}", write_need_time);
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
