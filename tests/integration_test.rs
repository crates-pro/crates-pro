#[cfg(test)]
mod integration_tests {

    use serial_test::serial;
    use tudriver::tugraph_client::TuGraphClient; // Assuming this is the client module/library you are testing
    use std::env;

    #[tokio::test]
    #[serial]
    async fn test_tugraph_setup() {
        // Instantiate the TuGraphClient for testing

        let tugraph_bolt_url = env::var("TUGRAPH_BOLT_URL").unwrap();
        let tugraph_user_name = env::var("TUGRAPH_USER_NAME").unwrap();
        let tugraph_user_password = env::var("TUGRAPH_USER_PASSWORD").unwrap();
        let tugraph_cratespro_db = env::var("TUGRAPH_CRATESPRO_DB").unwrap();

        let origin_client = TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, "")
            .await
            .unwrap();

        let origin_graphs = origin_client.list_graphs().await.unwrap();

        // check whether 'cratespro' exists
        if !origin_graphs.contains(&tugraph_cratespro_db) {
            origin_client.create_subgraph(&tugraph_cratespro_db).await.unwrap();
        }

        let client =
            TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, &tugraph_cratespro_db)
                .await
                .unwrap();

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
                "/workspace/target/debug/libplugin1.so",
            )
            .await
            .unwrap();

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();

        println!("All the loaded plugins: {:?}", plugins);

        let labels = client.list_edge_labels().await.unwrap();
        println!("labels: {}", labels);

        if !plugins.is_empty() {
            let pinfo = client
                .get_plugin_info("CPP", &plugins[0], false)
                .await
                .unwrap();
            println!("The first plugin: {:?}", pinfo);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_plugin1() {
        // Instantiate the TuGraphClient for testing

        let tugraph_bolt_url = env::var("TUGRAPH_BOLT_URL").unwrap();
        let tugraph_user_name = env::var("TUGRAPH_USER_NAME").unwrap();
        let tugraph_user_password = env::var("TUGRAPH_USER_PASSWORD").unwrap();
        let tugraph_cratespro_db = env::var("TUGRAPH_CRATESPRO_DB").unwrap();

        let origin_client = TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, "")
            .await
            .unwrap();

        let origin_graphs = origin_client.list_graphs().await.unwrap();
        println!("origin database contains graphs: {:?}", origin_graphs);

        // check whether "${TUGRAPH_CRATESPRO_DB}" exists
        if !origin_graphs.contains(&tugraph_cratespro_db) {
            println!("create graph: {}", tugraph_cratespro_db);
            origin_client.create_subgraph(&tugraph_cratespro_db).await.unwrap();
        }

        let client =
            TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, &tugraph_cratespro_db)
                .await
                .unwrap();

        let graphs = client.list_graphs().await.unwrap();
        println!("Current database contains graphs: {:?}", graphs);

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();
        println!("Current database contains plugins: {:?}", plugins);

        for plugin in plugins {
            client.delete_plugin("CPP", &plugin).await.unwrap();
        }

        client
            .load_plugin(
                "trace_dependencies1",
                "/workspace/target/debug/libplugin1.so",
            )
            .await
            .unwrap();

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();

        println!("All the loaded plugins: {:?}", plugins);

        if !plugins.is_empty() {
            let pinfo = client
                .get_plugin_info("CPP", &plugins[0], false)
                .await
                .unwrap();
            println!("The first plugin: {:?}", pinfo);
        }

        let labels = client.list_edge_labels().await.unwrap();
        println!("labels: {}", labels);

        let result = client
            .call_plugin(
                "CPP",
                "trace_dependencies1",
                "astroport-staking 2.0.0,astroport-circular-buffer 0.2.0",
                1.2,
                false,
            )
            .await
            .unwrap();

        println!("{:?}", result);
    }

    #[tokio::test]
    #[serial]
    async fn test_plugin2() {
        // Instantiate the TuGraphClient for testing

        let tugraph_bolt_url = env::var("TUGRAPH_BOLT_URL").unwrap();
        let tugraph_user_name = env::var("TUGRAPH_USER_NAME").unwrap();
        let tugraph_user_password = env::var("TUGRAPH_USER_PASSWORD").unwrap();
        let tugraph_cratespro_db = env::var("TUGRAPH_CRATESPRO_DB").unwrap();

        let origin_client = TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, "")
            .await
            .unwrap();

        let origin_graphs = origin_client.list_graphs().await.unwrap();
        println!("origin database contains graphs: {:?}", origin_graphs);

        // check whether "${TUGRAPH_CRATESPRO_DB}" exists
        if !origin_graphs.contains(&tugraph_cratespro_db) {
            println!("create graph: {}", tugraph_cratespro_db);
            origin_client.create_subgraph(&tugraph_cratespro_db).await.unwrap();
        }

        let client =
            TuGraphClient::new(&tugraph_bolt_url, &tugraph_user_name, &tugraph_user_password, &tugraph_cratespro_db)
                .await
                .unwrap();

        let graphs = client.list_graphs().await.unwrap();
        println!("Current database contains graphs: {:?}", graphs);

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();
        println!("Current database contains plugins: {:?}", plugins);

        for plugin in plugins {
            client.delete_plugin("CPP", &plugin).await.unwrap();
        }

        client
            .load_plugin(
                "trace_dependencies2",
                "/workspace/target/debug/libplugin2.so",
            )
            .await
            .unwrap();

        let plugins = client.list_plugin("CPP", "v1").await.unwrap();

        println!("All the loaded plugins: {:?}", plugins);

        if !plugins.is_empty() {
            let pinfo = client
                .get_plugin_info("CPP", &plugins[0], false)
                .await
                .unwrap();
            println!("The first plugin: {:?}", pinfo);
        }

        let labels = client.list_edge_labels().await.unwrap();
        println!("labels: {}", labels);

        let result = client
            .call_plugin(
                "CPP",
                "trace_dependencies2",
                "astroport-circular-buffer 0.2.0",
                1.2,
                false,
            )
            .await
            .unwrap();

        let result: Result<Vec<(i64, String, i64)>, _> = serde_json::from_str(&result.1);

        println!("{:#?}", result);
    }
}
