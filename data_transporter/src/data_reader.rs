use model::tugraph_model::{
    Application, ApplicationVersion, Library, LibraryVersion, Program, UProgram, UVersion,
};
use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    error::Error,
};
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
    async fn new_get_direct_dependency_nodes(
        &self,
        namespace: &str,
        nameversion: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    #[allow(dead_code)]
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
    async fn new_get_direct_dependent_nodes(
        &self,
        namespace: &str,
        nameversion: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;
    async fn get_indirect_dependent_nodes(
        &self,
        nameversion: NameVersion,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>>;

    //async fn get_max_version(&self, name: String) -> Result<String, Box<dyn Error>>;
    #[allow(dead_code)]
    async fn get_lib_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>>;
    async fn new_get_lib_version(
        &self,
        namespace: String,
        name: String,
    ) -> Result<Vec<String>, Box<dyn Error>>;
    #[allow(dead_code)]
    async fn get_app_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>>;
    async fn new_get_app_version(
        &self,
        namespace: String,
        name: String,
    ) -> Result<Vec<String>, Box<dyn Error>>;
    async fn get_all_dependencies(
        &self,
        nameversion: NameVersion,
    ) -> Result<HashSet<String>, Box<dyn Error>>;
    async fn new_get_all_dependencies(
        &self,
        namespace: String,
        nameversion: String,
    ) -> Result<HashSet<String>, Box<dyn Error>>;
    #[allow(dead_code)]
    async fn get_all_dependents(
        &self,
        nameversion: NameVersion,
    ) -> Result<HashSet<String>, Box<dyn Error>>;
    async fn new_get_all_dependents(
        &self,
        namespace: String,
        nameversion: String,
    ) -> Result<HashSet<String>, Box<dyn Error>>;
    async fn get_github_url(
        &self,
        namespace: String,
        name: String,
    ) -> Result<String, Box<dyn Error>>;
    async fn get_doc_url(&self, namespace: String, name: String) -> Result<String, Box<dyn Error>>;
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
    async fn get_github_url(
        &self,
        namespace: String,
        name: String,
    ) -> Result<String, Box<dyn Error>> {
        println!("{}|{}", namespace.clone(), name.clone());
        let query = format!(
            "
            MATCH (n:program {{namespace:'{}'}}) WHERE n.name='{}'
RETURN n.github_url 
        ",
            &namespace, &name
        );
        let results = self.client.exec_query(&query).await?;
        let mut res = vec![];
        for node in results {
            res.push(node);
        }
        let unique_items: HashSet<String> = res.clone().into_iter().collect();
        let mut nodes = vec![];
        for res in unique_items {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(url) = parsed.get("n.github_url").and_then(|v| v.as_str()) {
                nodes.push(url.to_string());
            }
        }
        Ok(nodes[0].clone())
    }
    async fn get_doc_url(&self, namespace: String, name: String) -> Result<String, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (n:program {{namespace:'{}'}}) WHERE n.name='{}'
RETURN n.doc_url 
        ",
            &namespace, &name
        );
        let results = self.client.exec_query(&query).await?;
        let mut res = vec![];
        for node in results {
            res.push(node);
        }
        let unique_items: HashSet<String> = res.clone().into_iter().collect();
        let mut nodes = vec![];
        for res in unique_items {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(url) = parsed.get("n.doc_url").and_then(|v| v.as_str()) {
                nodes.push(url.to_string());
            }
        }
        Ok(nodes[0].clone())
    }
    async fn get_all_dependencies(
        &self,
        nameversion: NameVersion,
    ) -> Result<HashSet<String>, Box<dyn Error>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let name_and_version = nameversion.name.clone() + "/" + &nameversion.version.clone();
        queue.push_back(name_and_version.to_string());

        while let Some(current) = queue.pop_front() {
            // 如果当前库没有被访问过
            if visited.insert(current.clone()) {
                // 获取当前库的直接依赖
                for dep in self.get_direct_dependency_nodes(&current).await.unwrap() {
                    let tmp = dep.name.clone() + "/" + &dep.version.clone();
                    queue.push_back(tmp);
                }
            }
        }
        visited.remove(&name_and_version);

        Ok(visited)
    }
    async fn new_get_all_dependencies(
        &self,
        namespace: String,
        nameversion: String,
    ) -> Result<HashSet<String>, Box<dyn Error>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        for node in self
            .new_get_direct_dependency_nodes(&namespace, &nameversion)
            .await
            .unwrap()
        {
            let nameandversion = node.clone().name + "/" + &node.clone().version;
            queue.push_back(nameandversion.clone());
        }
        //queue.push_back(name_and_version.to_string());
        let mut count = 0;
        while let Some(current) = queue.pop_front() {
            // 如果当前库没有被访问过
            count += 1;
            if count == 500 {
                break;
            }
            if visited.insert(current.clone()) {
                // 获取当前库的直接依赖
                for dep in self.get_direct_dependency_nodes(&current).await.unwrap() {
                    let tmp = dep.name.clone() + "/" + &dep.version.clone();
                    queue.push_back(tmp);
                }
            }
        }
        //visited.remove(&name_and_version);

        Ok(visited)
    }
    async fn get_all_dependents(
        &self,
        nameversion: NameVersion,
    ) -> Result<HashSet<String>, Box<dyn Error>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let name_and_version = nameversion.name.clone() + "/" + &nameversion.version.clone();
        queue.push_back(name_and_version.to_string());

        while let Some(current) = queue.pop_front() {
            // 如果当前库没有被访问过
            if visited.insert(current.clone()) {
                // 获取当前库的直接依赖
                for dep in self.get_direct_dependent_nodes(&current).await.unwrap() {
                    let tmp = dep.name.clone() + "/" + &dep.version.clone();
                    queue.push_back(tmp);
                }
            }
        }
        visited.remove(&name_and_version);

        Ok(visited)
    }
    async fn new_get_all_dependents(
        &self,
        namespace: String,
        nameversion: String,
    ) -> Result<HashSet<String>, Box<dyn Error>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        for node in self
            .new_get_direct_dependent_nodes(&namespace, &nameversion)
            .await
            .unwrap()
        {
            let nameandversion = node.clone().name + "/" + &node.clone().version;
            queue.push_back(nameandversion.clone());
        }
        //queue.push_back(name_and_version.to_string());
        //let mut count = 0;
        let len = queue.len();
        if len < 500 {
            while let Some(current) = queue.pop_front() {
                // 如果当前库没有被访问过
                if visited.insert(current.clone()) {
                    // 获取当前库的直接依赖
                    for dep in self.get_direct_dependent_nodes(&current).await.unwrap() {
                        let tmp = dep.name.clone() + "/" + &dep.version.clone();
                        queue.push_back(tmp);
                    }
                }
            }
        }
        //visited.remove(&name_and_version);

        Ok(visited)
    }
    async fn get_all_programs_id(&self) -> Vec<String> {
        //tracing::info!("start test ping");
        //self.client.test_ping().await;
        //tracing::info!("end test ping");
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
        //println!("enter get_direct_dependency_nodes");
        let query = format!(
            "
                MATCH (n:version {{name_and_version: '{}'}})-[:depends_on]->(m:version)
                RETURN m.name_and_version as name_and_version
                ",
            name_and_version
        );

        let results = self.client.exec_query(&query).await?;
        //println!("query success of get_direct_dependency_nodes");
        let unique_items: HashSet<String> = results.clone().into_iter().collect();
        let mut nodes = vec![];
        //println!("{:?}", results);

        for result in unique_items {
            let result_json: Value = serde_json::from_str(&result).unwrap();
            let name_version_str: String =
                serde_json::from_value(result_json["name_and_version"].clone()).unwrap();

            if let Some(name_version) = crate::NameVersion::from_string(&name_version_str) {
                nodes.push(name_version);
            }
        }

        Ok(nodes)
    }
    async fn new_get_direct_dependency_nodes(
        &self,
        namespace: &str,
        nameversion: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        println!("enter get_direct_dependency_nodes");
        let query1 = format!(
            "
                MATCH (p:program {{namespace: '{}'}})-[:has_type]->(l)-[:has_version]->(lv {{name_and_version: '{}'}})-[:has_dep_version]->(vs:version)-[:depends_on]->(m:version)
RETURN m.name_and_version as name_and_version
                ",
            namespace,
            nameversion,
        );
        let results1 = self.client.exec_query(&query1).await?;
        println!("finish get_direct_dep");
        let mut res = vec![];
        for node in results1 {
            res.push(node);
        }
        let unique_items: HashSet<String> = res.clone().into_iter().collect();
        //println!("query success of get_direct_dependency_nodes");
        let mut nodes = vec![];
        //println!("{:?}", results);

        for result in unique_items {
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
            println!("{} {}", node.clone().name, node.clone().version);
            let new_nodes = self.get_indirect_dependency_nodes(node).await.unwrap();
            for new_node in new_nodes {
                println!("{} {}", new_node.clone().name, new_node.clone().version);
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
        /*let name_and_version = nameversion.name + "/" + &nameversion.version;
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
        Ok(node_count)*/

        let all_nodes = self.get_all_dependencies(nameversion).await.unwrap();
        Ok(all_nodes.len())
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
        let unique_items: HashSet<String> = results.clone().into_iter().collect();
        let mut nodes = vec![];
        //println!("{:?}", results);

        for result in unique_items {
            let result_json: Value = serde_json::from_str(&result).unwrap();
            let name_version_str: String =
                serde_json::from_value(result_json["name_and_version"].clone()).unwrap();

            if let Some(name_version) = crate::NameVersion::from_string(&name_version_str) {
                nodes.push(name_version);
            }
        }

        Ok(nodes)
    }
    async fn new_get_direct_dependent_nodes(
        &self,
        namespace: &str,
        nameversion: &str,
    ) -> Result<Vec<crate::NameVersion>, Box<dyn Error>> {
        //println!("enter get_direct_dependency_nodes");
        let query1 = format!(
            "
                MATCH (p:program {{namespace: '{}'}})-[:has_type]->(l)-[:has_version]->(lv {{name_and_version:'{}'}})-[:has_dep_version]->(vs:version)<-[:depends_on]-(m:version)
RETURN m.name_and_version as name_and_version
                ",
            namespace,
            nameversion
        );
        let results1 = self.client.exec_query(&query1).await?;
        let mut res = vec![];
        for node in results1 {
            res.push(node);
        }
        let unique_items: HashSet<String> = res.clone().into_iter().collect();
        //println!("query success of get_direct_dependency_nodes");
        let mut nodes = vec![];
        //println!("{:?}", results);

        for result in unique_items {
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
    /*async fn get_max_version(&self, name: String) -> Result<String, Box<dyn Error>> {
        let query = format!(
            "
                 MATCH (n:program {{name:'{}'}}) RETURN n.max_version LIMIT 1
                ",
            name
        );
        let results = self.client.exec_query(&query).await?;
        let max_version = &results[0];
        Ok(max_version.to_string())
    }*/

    async fn get_lib_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (n:library_version {{name: '{}'}}) RETURN n.version LIMIT 100",
            name
        );
        //let starttime = Instant::now();
        let results = self.client.exec_query(&query).await.unwrap();
        //let endtime = starttime.elapsed();
        //println!("query need time:{:?}", endtime);
        let mut realres = vec![];
        //let starttime2 = Instant::now();
        for res in results {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(version) = parsed.get("n.version").and_then(|v| v.as_str()) {
                //println!("Version: {}", version);
                realres.push(version.to_string());
            } else {
                //println!("Version not found");
            }
        }
        //let endtime2 = starttime2.elapsed();
        //println!("rest query need time:{:?}", endtime2);
        Ok(realres)
    }
    async fn new_get_lib_version(
        &self,
        namespace: String,
        name: String,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (p:program {{namespace: '{}'}})-[:has_type]->(l)-[:has_version]->(lv {{name:'{}'}})
RETURN lv.version",
            namespace,
            name,
        );
        //let starttime = Instant::now();
        let results = self.client.exec_query(&query).await.unwrap();
        let unique_items: HashSet<String> = results.clone().into_iter().collect();
        //let endtime = starttime.elapsed();
        //println!("query need time:{:?}", endtime);
        let mut realres = vec![];
        //let starttime2 = Instant::now();
        for res in unique_items {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(version) = parsed.get("lv.version").and_then(|v| v.as_str()) {
                //println!("Version: {}", version);
                realres.push(version.to_string());
            } else {
                //println!("Version not found");
            }
        }
        //let endtime2 = starttime2.elapsed();
        //println!("rest query need time:{:?}", endtime2);
        Ok(realres)
    }
    async fn get_app_version(&self, name: String) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (n:application_version {{name: '{}'}}) RETURN n.version LIMIT 100",
            name
        );
        let results = self.client.exec_query(&query).await.unwrap();
        let mut realres = vec![];
        for res in results {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(version) = parsed.get("n.version").and_then(|v| v.as_str()) {
                //println!("Version: {}", version);
                realres.push(version.to_string());
            } else {
                //println!("Version not found");
            }
        }
        Ok(realres)
    }
    async fn new_get_app_version(
        &self,
        namespace: String,
        name: String,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (p:program {{namespace: '{}'}})-[:has_type]->(a:application)-[:has_version]->(av:application_version {{name:'{}'}})
RETURN av.version",
            namespace,
            name,
        );
        //let starttime = Instant::now();
        let results = self.client.exec_query(&query).await.unwrap();
        let unique_items: HashSet<String> = results.clone().into_iter().collect();
        //let endtime = starttime.elapsed();
        //println!("query need time:{:?}", endtime);
        let mut realres = vec![];
        //let starttime2 = Instant::now();
        for res in unique_items {
            let parsed: Value = serde_json::from_str(&res).unwrap();
            if let Some(version) = parsed.get("av.version").and_then(|v| v.as_str()) {
                //println!("Version: {}", version);
                realres.push(version.to_string());
            } else {
                //println!("Version not found");
            }
        }
        //let endtime2 = starttime2.elapsed();
        //println!("rest query need time:{:?}", endtime2);
        Ok(realres)
    }
}
