use model::tugraph_model::{
    Application, ApplicationVersion, Library, LibraryVersion, Program, UProgram, UVersion,
};
use serde_json::Value;
use std::error::Error;
use tudriver::tugraph_client::TuGraphClient;

use async_trait::async_trait;

use crate::NameVersion;

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
    async fn get_direct_dependency_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    async fn get_program_by_name(&self, program_name: &str)
        -> Result<Vec<Program>, Box<dyn Error>>;
    async fn get_indirect_dependency_nodes(
        &self,
        nameversion: NameVersion,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    async fn count_dependencies(&self, nameversion: NameVersion) -> Result<usize, Box<dyn Error>>;
    async fn get_direct_dependent_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    async fn get_indirect_dependent_nodes(
        &self,
        nameversion: NameVersion,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    async fn get_max_version(&self, name: String) -> Result<String, Box<dyn Error>>;
    async fn get_lib_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>>;
    async fn get_app_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>>;
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
        //tracing::info!("start test ping");
        //self.client.test_ping().await;
        //tracing::info!("end test ping");
        let query = "
            MATCH (p: program)
            RETURN p 
            LIMIT 100
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
            let dependencies = self
                .get_direct_dependency_nodes(&name_version)
                .await
                .unwrap();

            versions.push(crate::VersionInfo {
                version_base,
                dependencies,
            })
        }
        Ok(versions)
    }

    async fn get_direct_dependency_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        let query = format!(
            "
                MATCH (n:version {{name_and_version: '{}'}})-[:depends_on]->(m:version)
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
    async fn get_indirect_dependency_nodes(
        &self,
        nameversion: NameVersion,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        let name_and_version = nameversion.name + "/" + &nameversion.version;
        let mut nodes = self
            .get_direct_dependency_nodes(&name_and_version)
            .await
            .unwrap();
        for node in nodes.clone() {
            let new_nodes = self.get_indirect_dependency_nodes(node).await.unwrap();
            for new_node in new_nodes {
                nodes.push(new_node);
            }
        }
        Ok(nodes)
    }
    async fn get_program_by_name(
        &self,
        program_name: &str,
    ) -> Result<Vec<Program>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (p:program)
            WHERE p.name CONTAINS '{}'
            RETURN p
            ",
            program_name
        );
        let results = self.client.exec_query(&query).await?;
        let mut programs = vec![];
        for result in results {
            let programs_json: Value = serde_json::from_str(&result).unwrap();
            let pro = programs_json["p"].clone();
            let program: Program = serde_json::from_value(pro).unwrap();
            programs.push(program);
        }
        Ok(programs)
    }
    async fn count_dependencies(&self, nameversion: NameVersion) -> Result<usize, Box<dyn Error>> {
        let name_and_version = nameversion.name + "/" + &nameversion.version;
        let mut all_nodes = self
            .get_direct_dependency_nodes(&name_and_version)
            .await
            .unwrap();
        for node in all_nodes.clone() {
            let nodes = self.get_indirect_dependency_nodes(node).await.unwrap();
            for indirect_node in nodes {
                all_nodes.push(indirect_node);
            }
        }
        let node_count = all_nodes.len();
        Ok(node_count)
    }
    async fn get_direct_dependent_nodes(
        &self,
        name_and_version: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        let query = format!(
            "
                MATCH (n:version {{name_and_version: '{}'}})<-[:depends_on]-(m:version)
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
    async fn get_indirect_dependent_nodes(
        &self,
        nameversion: NameVersion,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        let name_and_version = nameversion.name + "/" + &nameversion.version;
        let mut nodes = self
            .get_direct_dependent_nodes(&name_and_version)
            .await
            .unwrap();
        for node in nodes.clone() {
            let new_nodes = self.get_indirect_dependent_nodes(node).await.unwrap();
            for new_node in new_nodes {
                nodes.push(new_node);
            }
        }
        Ok(nodes)
    }
    async fn get_max_version(&self, name: String) -> Result<String, Box<dyn Error>> {
        let query = format!(
            "
                 MATCH (n:program {{name:'{}'}}) RETURN n.max_version LIMIT 1
                ",
            name
        );
        let results = self.client.exec_query(&query).await?;
        let max_version = &results[0];
        Ok(max_version.to_string())
    }
    async fn get_lib_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (n:library_version {{name: '{}'}}) RETURN n.version LIMIT 100",
            name
        );
        let results = self.client.exec_query(&query).await.unwrap();
        Ok(results)
    }
    async fn get_app_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (n:application_version {{name: '{}'}}) RETURN n.version LIMIT 100",
            name
        );
        let results = self.client.exec_query(&query).await.unwrap();
        Ok(results)
    }
}
