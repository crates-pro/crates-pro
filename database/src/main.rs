use std::{error::Error, future, sync::Arc};

use database::quary_server::Server;

use tokio::signal;

use tokio::time::{self, Duration};

const SLEEP_TIME_SECS: Option<u64> = Some(20); // The opening time for server

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server = Arc::new(Server::new());

    let server_clone = Arc::clone(&server);

    // start
    tokio::spawn(async move {
        if let Err(e) = server_clone.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // wait for time or infinitely
    match SLEEP_TIME_SECS {
        Some(secs) => {
            let sleep_duration = Duration::from_secs(secs);
            tokio::select! {
                _ = time::sleep(sleep_duration) => {
                    println!("Time to close the server after waiting for {} seconds.", secs);
                }
                _ = signal::ctrl_c() => {
                    println!("Received Ctrl+C - shutting down.");
                }
            }
        }
        None => {
            println!("No timeout set; the server will run indefinitely. Press Ctrl+C to stop.");
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        }
    }

    // close the server
    if let Err(e) = server.close().await {
        eprintln!("Failed to close the server gracefully: {}", e);
    } else {
        println!("Server closed gracefully.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use neo4rs::*;
    use tokio;

    /// This is the test to test whether the Tugraph is setup.
    #[tokio::test]
    async fn test_tugraph_setup() {
        // build bolt config
        let default_graph_config = ConfigBuilder::default()
            .uri("bolt://localhost:7687")
            .user("admin")
            .password("73@TuGraph")
            .db("default")
            .build()
            .unwrap();

        // connect the database
        let default_graph = Graph::connect(default_graph_config).await.unwrap();

        let _ = default_graph
            .run(query(
                "CALL dbms.graph.createGraph('graph_for_test', 'description', 2045)",
            ))
            .await;

        let config = ConfigBuilder::default()
            .uri("bolt://localhost:7687")
            .user("admin")
            .password("73@TuGraph")
            .db("graph_for_test")
            .build()
            .unwrap();

        let graph = Graph::connect(config).await.unwrap();

        // 注意：在每次测试前后清理数据库是一个好习惯，
        // 这里假设`graph.run`能正确处理数据库操作。
        graph.run(query("CALL db.dropDB()")).await.unwrap();
        graph.run(query("CALL db.createVertexLabel('person', 'id' , 'id' ,INT32, false, 'name' ,STRING, false)")).await.unwrap();
        graph
            .run(query(
                "CALL db.createEdgeLabel('is_friend','[[\"person\",\"person\"]]')",
            ))
            .await
            .unwrap();
        graph
            .run(query(
                "create (n1:person {name:'jack',id:1}), (n2:person {name:'lucy',id:2})",
            ))
            .await
            .unwrap();
        graph
            .run(query(
                "match (n1:person {id:1}), (n2:person {id:2}) create (n1)-[r:is_friend]->(n2)",
            ))
            .await
            .unwrap();
        let mut result = graph
            .execute(query("match (n)-[r]->(m) return n,r,m"))
            .await
            .unwrap();

        // 这里可以添加具体的断言来校验`n`, `r`, `m`的值，例如：
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

        // 测试后的清理可以在这里进行
    }
}
