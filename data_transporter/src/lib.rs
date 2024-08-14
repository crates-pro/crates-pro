use model::tugraph_model::Program;
use serde_json::Value;
use std::error::Error;
use tudriver::tugraph_client::TuGraphClient;

pub struct DataReader {
    client: TuGraphClient,
}

impl DataReader {
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
        let query = "
            MATCH (p: program)
            RETURN p
        ";

        let result = self.client.exec_query(query).await?;
        let programs_json: Vec<Value> = serde_json::from_str(&result)?;

        let programs: Vec<Program> = programs_json
            .into_iter()
            .map(|program| {
                let properties = program["properties"].clone();
                serde_json::from_value(properties).unwrap()
            })
            .collect();

        Ok(programs)
    }

    pub async fn get_related_nodes(&self, program_id: &str) -> Result<Value, Box<dyn Error>> {
        let query = format!(
            "
            MATCH (p:Program {{ id: '{}' }})
            OPTIONAL MATCH (p)-[r:HasType|HasVersion|HasDepVersion|DependsOn]->(related)
            RETURN p, r, related
            ",
            program_id
        );

        let result = self.client.exec_query(&query).await?;
        let related_nodes: Value = serde_json::from_str(&result)?;
        Ok(related_nodes)
    }
}
