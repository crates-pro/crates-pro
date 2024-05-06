use neo4rs::*;
use serde_json::Map;
use std::error::Error;
use std::fs::File;
use std::io::Read;

/// doc: https://tugraph-db.readthedocs.io/zh-cn/latest/5.developer-manual/6.interface/1.query/1.cypher.html
/// https://github.com/TuGraph-family/tugraph-db/blob/master/src/cypher/procedure/procedure.h
pub struct TuGraphClient {
    graph: Graph,
}

impl TuGraphClient {
    /// Initialize TuGraph Client
    /// Arguments:
    /// * `url`: url for bolt
    /// * `user`: user name
    /// * `password`: password for user
    /// * `db`: graph, default is 'default'
    pub async fn new(
        uri: &str,
        user: &str,
        password: &str,
        db: &str,
    ) -> Result<Self, Box<dyn Error>> {
        let config = ConfigBuilder::default()
            .uri(uri)
            .user(user)
            .password(password)
            .db(db)
            .build()?;

        let graph = Graph::connect(config).await?;
        Ok(TuGraphClient { graph })
    }

    /// Reset the database, be carefully
    #[allow(dead_code)]
    pub(crate) async unsafe fn drop_database(&self) -> Result<(), Box<dyn Error>> {
        self.graph.run(query("CALL db.dropDB()")).await?;
        Ok(())
    }

    /// Creates a vertex label in the database.
    ///
    /// Arguments:
    /// * `label_name`: name of vertex label
    /// * `primary`: primary field of vertex label
    /// * `field_specs`: A slice of tuples where each tuple represents field_spec in the form of (property_name, property_type, is_option).
    ///
    /// Returns:
    /// * `Result<(), Box<dyn Error>>` - Ok(()) if successful, or an error wrapped in Box<dyn Error> otherwise.
    /// Example usage: Create a `person` vertex label with an ID of type INT32 and additional properties for `name` and `age`.
    /// ```ignore
    ///     client.create_vertex_label("person", "id",  &[("name".to_string(), "STRING".to_string(), false), ("age".to_string(), "INT32".to_string(), false)]).await.unwrap();
    /// ```
    pub async fn create_vertex_label(
        &self,
        label_name: &str,
        primary_field: &str,
        field_specs: &[(String, String, bool)],
    ) -> Result<(), Box<dyn Error>> {
        let mut fields_string = field_specs
            .iter()
            .map(|(name, type_, option)| format!("'{}', {}, {}", name, type_, option))
            .collect::<Vec<_>>()
            .join(", ");

        fields_string =
            if !fields_string.is_empty() { ", " } else { "" }.to_string() + &fields_string;

        let query_string = format!(
            "CALL db.createVertexLabel('{}', '{}'{})",
            label_name, primary_field, fields_string
        );
        println!("Query: {}", query_string);
        self.graph.run(query(&query_string)).await?;
        Ok(())
    }

    /// Creates an edge label in the database.
    ///
    /// Arguments:
    /// * `label_name`: Name of the edge label.
    /// * `edge_constraints`: Vec of tuple pairs, each representing a valid start and end vertex label for the edge.
    /// * `field_specs`: A slice of tuples where each tuple represents a field_spec in the form of (field_name, field_type, is_optional).
    ///
    /// Returns:
    /// * `Result<(), Box<dyn Error>>` - Ok(()) if successful, or an error wrapped in Box<dyn Error> otherwise.
    ///
    /// Example usage: Create a `KNOWS` edge label with constraints that it can only exist between `Person` and `Person` or `Person` and `Organization`, and with an optional `name` property of type `int32`.
    /// ```ignore
    ///     client.create_edge_label(
    ///         "KNOWS".to_string(),
    ///         &[("Person".to_string(), "Person".to_string()), ("Person".to_string(), "Organization".to_string())],
    ///         &[("name".to_string(), "INT32".to_string(), true)]
    ///     ).await.unwrap();
    /// ```
    pub async fn create_edge_label(
        &self,
        label_name: String,
        edge_constraints: &[(String, String)],
        field_specs: &[(String, String, bool)],
    ) -> Result<(), Box<dyn Error>> {
        let constraint_strings = edge_constraints
            .iter()
            .map(|(start_label, end_label)| format!("[\"{}\", \"{}\"]", start_label, end_label))
            .collect::<Vec<_>>()
            .join(", ");

        let mut fields_string = field_specs
            .iter()
            .map(|(name, type_, optional)| {
                let option_str = if *optional { "true" } else { "false" };
                format!("'{}', '{}', {}", name, type_, option_str)
            })
            .collect::<Vec<_>>()
            .join(", ");
        fields_string =
            if !fields_string.is_empty() { ", " } else { "" }.to_string() + &fields_string;

        let query_string = format!(
            "CALL db.createEdgeLabel('{}', '[{}]'{})",
            label_name, constraint_strings, fields_string
        );

        println!("Query: {}", query_string);
        self.graph.run(query(&query_string)).await?;
        Ok(())
    }

    /// Loads a plugin into the database.
    ///
    /// Arguments:
    /// * `plugin_name`: The name of the plugin as a STRING.
    /// * `plugin_so_path`: The path of the plugin as a STRING.
    ///
    /// Returns:
    /// * `Result<(), Box<dyn Error>>` - Ok(()) if successful, or an error wrapped in Box<dyn Error> otherwise.
    ///
    /// Example usage: Load a custom `HelloWorld` plugin.
    /// ```ignore
    ///     client.load_plugin("trace_dependencies", "../trace_dependencies.so").await.unwrap();
    /// ```
    pub async fn load_plugin(
        &self,
        plugin_name: &str,
        plugin_so_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        let plugin_type: &str = "CPP";
        let plugin_description: &str = "plugin";
        let read_only: bool = true;
        let version: &str = "v1";
        let code_type: &str = "SO";

        let mut file = File::open(plugin_so_path)?;
        let mut buffer = Vec::new();

        file.read_to_end(&mut buffer)?;

        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        let engine = STANDARD;

        let plugin_content: &str = &engine.encode(buffer);

        let query_string = format!(
            "CALL db.plugin.loadPlugin('{}', '{}', '{}', '{}', '{}', {}, '{}')",
            plugin_type,
            plugin_name,
            plugin_content,
            code_type,
            plugin_description,
            read_only,
            version
        );

        self.graph.run(query(&query_string)).await.unwrap();

        println!("load plugin {}", plugin_name);
        Ok(())
    }

    /// list the info of loaded plugins
    ///
    /// # param
    /// * `plugin_type` -
    /// * `plugin_version` -
    pub async fn list_plugin(
        &self,
        plugin_type: &str,
        plugin_version: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let query_string = format!(
            "CALL db.plugin.listPlugin('{}', '{}')",
            plugin_type, plugin_version
        );

        let mut result = self.graph.execute(query(&query_string)).await?;
        // println!("{:?}", result.next().await);
        let mut plugins = Vec::new();

        while let Some(row) = result.next().await? {
            let desc: Map<_, _> = row.get("plugin_description").unwrap();
            let name = desc
                .get("name")
                .unwrap()
                .to_string()
                .trim_matches('"')
                .to_string();
            plugins.push(name);
        }
        Ok(plugins)
    }

    /// Deletes a plugin based on its type and name.
    ///
    /// # Parameters
    /// * `plugin_type` - The type of the plugin to delete (e.g., "CPP", "PY").
    /// * `plugin_name` - The name of the plugin to delete.
    ///
    /// # Returns
    /// Returns `Ok(())` if the plugin was successfully deleted, or an error if the operation fails.
    ///
    /// # Errors
    /// Returns an error if the delete operation cannot be executed or if the database responds with an error.
    pub async fn delete_plugin(
        &self,
        plugin_type: &str,
        plugin_name: &str,
    ) -> Result<(), Box<dyn Error>> {
        let query_string = format!(
            "CALL db.plugin.deletePlugin('{}', '{}')",
            plugin_type, plugin_name
        );

        // Executes the query.
        // In a real-world scenario, you should handle potential errors properly,
        // e.g., if the plugin does not exist or if the arguments are invalid.
        self.graph.run(query(&query_string)).await?;
        println!("delete plugin {}", plugin_name);
        Ok(())
    }

    /// List all the subgraphs in the database
    pub async fn list_graphs(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let mut result = self
            .graph
            .execute(query(
                "CALL dbms.graph.listGraphs() YIELD graph_name RETURN graph_name",
            ))
            .await?;

        let mut names = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let name: String = row.get("graph_name")?;
            names.push(name);
        }

        Ok(names)
    }
}

/// unit tests
#[cfg(test)]
mod tests {

    use super::*;
    use tokio;

    /// This is the test to test whether the TuGraph server is setup.
    #[tokio::test]
    async fn test_tugraph_client() {
        // build bolt config
        let client_ = TuGraphClient::new("bolt://localhost:7687", "admin", "73@TuGraph", "default")
            .await
            .unwrap();

        let _ = client_
            .graph
            .run(query(
                "CALL dbms.graph.createGraph('t1', 'description', 2045)",
            ))
            .await;

        let client = TuGraphClient::new("bolt://localhost:7687", "admin", "73@TuGraph", "t1")
            .await
            .unwrap();

        unsafe {
            client.drop_database().await.unwrap();
        }

        let graphs = client.list_graphs().await.unwrap();
        println!("{:?}", graphs);

        client
            .create_vertex_label(
                "person",
                "id",
                &[
                    ("id".to_string(), "INT32".to_string(), false),
                    ("name".to_string(), "STRING".to_string(), false),
                ],
            )
            .await
            .unwrap();

        client
            .create_edge_label(
                "is_friend".to_string(),
                &[("person".to_string(), "person".to_string())],
                &[],
            )
            .await
            .unwrap();

        client
            .graph
            .run(query(
                "create (n1:person {name:'jack',id:1}), (n2:person {name:'lucy',id:2})",
            ))
            .await
            .unwrap();
        client
            .graph
            .run(query(
                "match (n1:person {id:1}), (n2:person {id:2}) create (n1)-[r:is_friend]->(n2)",
            ))
            .await
            .unwrap();
        let mut result = client
            .graph
            .execute(query("match (n)-[r]->(m) return n,r,m"))
            .await
            .unwrap();

        // check
        if let Ok(Some(row)) = result.next().await {
            let n: Node = row.get("n").unwrap();
            assert_eq!(n.id(), 0);
            let r: Relation = row.get("r").unwrap();
            assert_eq!(r.start_node_id(), 0);
            assert_eq!(r.end_node_id(), 1);
            let m: Node = row.get("m").unwrap();
            assert_eq!(m.id(), 1);
        } else {
            panic!("Error no result");
        }
    }

    /// This is the test to test whether the TuGraph server is setup.
    #[tokio::test]
    async fn test_tugraph_client_load_plugin() {
        // build bolt config
        let client_ = TuGraphClient::new("bolt://localhost:7687", "admin", "73@TuGraph", "default")
            .await
            .unwrap();

        let _ = client_
            .graph
            .run(query(
                "CALL dbms.graph.createGraph('t2', 'description', 2045)",
            ))
            .await;

        let client = TuGraphClient::new("bolt://localhost:7687", "admin", "73@TuGraph", "t2")
            .await
            .unwrap();

        // unsafe {
        //     client.drop_database().await.unwrap();
        // }

        let graphs = client.list_graphs().await.unwrap();
        println!("{:?}", graphs);

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();
        println!("{:?}", plugins);

        for plugin in plugins {
            client.delete_plugin("CPP", &plugin).await.unwrap();
        }

        client
            .load_plugin(
                "trace_dependencies1",
                "/workspace/target/release/libplugin1.so",
            )
            .await
            .unwrap();

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();
        assert!(plugins.len() == 1);
    }
}
