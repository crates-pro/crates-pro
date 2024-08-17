use model::tugraph_model::{
    Application, ApplicationVersion, Library, LibraryVersion, Program, UProgram, UVersion,
};
use serde_json::Value;
use std::error::Error;
use tudriver::tugraph_client::TuGraphClient;

pub struct DataReader {
    client: TuGraphClient,
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

    pub async fn get_all_programs(&self) -> Result<Vec<Program>, Box<dyn Error>> {
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

            let (_p, islib) = self.get_type(&program.id).await.unwrap();
            self.get_versions(&program.id, islib).await.unwrap();
            //println!("{:?}", program);
            programs.push(program);
        }

        Ok(programs)
    }

    pub async fn get_type(
        &self,
        program_id: &str,
    ) -> Result<(Vec<UProgram>, bool), Box<dyn Error>> {
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
            //println!("{}", label);

            let o = result_json["o"].clone();
            //println!("{:?}", result);
            if label.eq(&"library".to_string()) {
                islib = true;
                let library: Library = serde_json::from_value(o).unwrap();
                //println!("{:?}", library);
                uprograms.push(UProgram::Library(library));
            } else if label.eq(&"application".to_string()) {
                let application: Application = serde_json::from_value(o).unwrap();
                //println!("{:?}", application);

                uprograms.push(UProgram::Application(application));
            }
        }
        Ok((uprograms, islib))
    }

    pub async fn get_versions(
        &self,
        program_id: &str,
        is_lib: bool,
    ) -> Result<Vec<UVersion>, Box<dyn Error>> {
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

        let mut versions = vec![];
        for result in results {
            let result_json: Value = serde_json::from_str(&result).unwrap();

            let o = result_json["o"].clone();
            //println!("{:?}", result);
            if is_lib {
                let library_version: LibraryVersion = serde_json::from_value(o).unwrap();
                println!("{:?}", library_version);
                versions.push(UVersion::LibraryVersion(library_version));
            } else {
                let application_version: ApplicationVersion = serde_json::from_value(o).unwrap();
                println!("{:?}", application_version);

                versions.push(UVersion::ApplicationVersion(application_version));
            }
        }
        Ok(versions)
    }
}
