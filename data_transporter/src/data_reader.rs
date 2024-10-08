use model::tugraph_model::{
    Application, ApplicationVersion, Library, LibraryVersion, Program, UProgram, UVersion,
};
use serde_json::Value;
use std::error::Error;
use tudriver::tugraph_client::TuGraphClient;

use async_trait::async_trait;

#[async_trait]
pub trait DataReaderTrait: Send + Sync {
    async fn get_all_programs_id(&self) -> Vec<String>;
    async fn get_program(&self, program_id: &str) -> Result<Program, Box<dyn Error>>;
    async fn get_type(&self, program_id: &str) -> Result<(UProgram, bool), Box<dyn Error>>;
    async fn get_versions(
        &self,
        program_id: &str,
        is_lib: bool,
    ) -> Result<Vec<crate::VersionInfo>, Box<dyn Error>>;
    async fn get_dependency_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
}

#[derive(Clone)]
pub struct DataReader {
    pub client: TuGraphClient,
}

impl DataReader {
    /// let client_ =
    /// TuGraphClient::new("bolt://172.17.0.1:7687", "admin", "73@TuGraph", "default")
    /// .await
    /// .unwrap();
    pub async fn new(
        uri: &str,
        user: &str,
        password: &str,
        db: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let client = TuGraphClient::new(uri, user, password, db).await?;
        Ok(DataReader { client })
    }
}

#[async_trait]
impl DataReaderTrait for DataReader {
    async fn get_all_programs_id(&self) -> Vec<String> {
        self.client.test_ping().await;

        let query = "
            MATCH (p: program)
            RETURN p
        ";

        let results = self.client.exec_query(query).await.unwrap();

        let mut programs = vec![];
        for result in results {
            let programs_json: Value = serde_json::from_str(&result).unwrap();

            let pro = programs_json["p"].clone();
            //println!("{:#?}", pro);
            let program: Program = serde_json::from_value(pro).unwrap();

            programs.push(program.id);
        }

        programs
    }

    async fn get_program(&self, program_id: &str) -> Result<Program, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (p: program {{id: '{}'}})
            RETURN p            
            ",
            program_id
        );
        let results = self.client.exec_query(&query).await?;
        let programs_json: Value = serde_json::from_str(&results[0]).unwrap();
        let pro = programs_json["p"].clone();
        let program: Program = serde_json::from_value(pro).unwrap();
        Ok(program)
    }

    async fn get_type(&self, program_id: &str) -> Result<(UProgram, bool), Box<dyn Error>> {
        let mut islib = false;

        let query = format!(
            "
            MATCH (p: program {{id: '{}'}})-[:has_type]->(o)
            RETURN o, label(o) as o_label
            ",
            program_id
        );

        let results = self.client.exec_query(&query).await?;
        let mut uprograms = vec![];
        for result in results {
            let result_json: Value = serde_json::from_str(&result).unwrap();

            let label: String = serde_json::from_value(result_json["o_label"].clone()).unwrap();

            let o = result_json["o"].clone();
            if label.eq(&"library".to_string()) {
                islib = true;
                let library: Library = serde_json::from_value(o).unwrap();
                uprograms.push(UProgram::Library(library));
            } else if label.eq(&"application".to_string()) {
                let application: Application = serde_json::from_value(o).unwrap();
                uprograms.push(UProgram::Application(application));
            }
        }
        Ok((uprograms[0].clone(), islib))
    }

    async fn get_versions(
        &self,
        program_id: &str,
        is_lib: bool,
    ) -> Result<Vec<crate::VersionInfo>, Box<dyn Error>> {
        let query = if is_lib {
            format!(
                "
                MATCH (l: library {{id: '{}'}})-[:has_version]->(o)
                RETURN o
            ",
                program_id
            )
        } else {
            format!(
                "
                MATCH (l: application {{id: '{}'}})-[:has_version]->(o)
                RETURN o
                ",
                program_id
            )
        };

        let results = self.client.exec_query(&query).await?;

        let mut versions: Vec<crate::VersionInfo> = vec![];
        for result in results {
            let result_json: Value = serde_json::from_str(&result).unwrap();

            let o = result_json["o"].clone();
            //println!("{:?}", result);

            let (version_base, name_version) = if is_lib {
                let library_version: LibraryVersion = serde_json::from_value(o).unwrap();
                (
                    UVersion::LibraryVersion(library_version.clone()),
                    library_version.name_and_version.clone(),
                )
            } else {
                let application_version: ApplicationVersion = serde_json::from_value(o).unwrap();
                (
                    UVersion::ApplicationVersion(application_version.clone()),
                    application_version.name_and_version.clone(),
                )
            };
            tracing::debug!("Read version for id {}: {:?}", program_id, version_base);

            // get dependencies
            let dependencies = self.get_dependency_nodes(&name_version).await.unwrap();

            versions.push(crate::VersionInfo {
                version_base,
                dependencies,
            })
        }
        Ok(versions)
    }

    async fn get_dependency_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        let query = format!(
            "
                MATCH (n {{name_and_version: '{}'}})-[:depends_on]->(m)
                RETURN m.name_and_version as name_and_version
                ",
            name_and_version
        );

        let results = self.client.exec_query(&query).await?;
        let mut nodes = vec![];
        //println!("{:?}", results);

        for result in results {
            let result_json: Value = serde_json::from_str(&result).unwrap();
            let name_version_str: String =
                serde_json::from_value(result_json["name_and_version"].clone()).unwrap();

            if let Some(name_version) = crate::NameVersion::from_string(&name_version_str) {
                nodes.push(name_version);
            }
        }

        Ok(nodes)
    }
}
