use base64::encode as base64_encode;
use reqwest::{Client, StatusCode};
use serde::Serialize;
use std::fs::File;
use std::io::Read;

use std::{path::PathBuf, str::FromStr};
use tugraph::{db::OpenOptions, Error};

#[derive(Serialize)]
struct PluginData {
    name: String,
    code_base64: String,
    description: String,
    read_only: bool,
    code_type: String,
}

pub async fn load_plugin(plugin_path: &str) -> Result<(), reqwest::Error> {
    // import rust plugin.
    //open the local .so plugin.
    let mut file = File::open("./age_10.so").expect("Failed to open file");
    let mut contents = vec![];
    file.read_to_end(&mut contents)
        .expect("Failed to read file");
    // construct the plugin data
    let data = PluginData {
        name: "age_10".into(),
        code_base64: base64_encode(&contents),
        description: "Custom Page Rank Procedure".into(),
        read_only: true,
        code_type: "so".into(),
    };

    let client = Client::new();

    // load the plugin
    let post_res = client
        .post("http://127.0.0.1:7071/db/school/cpp_plugin/age_10")
        .json(&data)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    println!("POST Status: {}", post_res.status().as_u16());

    // list the plugins
    let client = Client::new();
    let res = client
        .get("http://127.0.0.1:7071/db/school/cpp_plugin")
        .send()
        .await?;

    println!(
        "GET Status: {}, plugins: {}",
        res.status(),
        res.text().await?
    );

    Ok(())
}
