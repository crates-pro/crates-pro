#[cfg(test)]
mod integration_tests {

    use tudriver::tugraph_client::TuGraphClient; // Assuming this is the client module/library you are testing

    #[tokio::test]
    async fn test_integration_flow() {
        // Instantiate the TuGraphClient for testing
        let client =
            TuGraphClient::new("bolt://172.17.0.1:7687", "admin", "73@TuGraph", "cratespro")
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
                "/workspace/target/release/libplugin1.so",
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

        let result = client
            .call_plugin(
                "CPP",
                "trace_dependencies1",
                "accesskit_winit 0.7.3,accesskit 0.8.1",
                1.2,
                false,
            )
            .await
            .unwrap();

        println!("{:?}", result);
    }
}
