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
    }
}
