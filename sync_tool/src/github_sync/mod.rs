use database::storage::Context;
use entity::programs;
use model::GitHubSearchResponse;
use reqwest::Error;
use sea_orm::{prelude::Uuid, ActiveValue::Set};

pub mod model;

pub async fn start_sync(context: &Context) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let mut page = 1;
    let per_page = 100;

    loop {
        let url = format!(
            "https://api.github.com/search/repositories?q=language:Rust&sort=stars&order=desc&page={}&per_page={}",
            page, per_page
        );

        let res = client
            .get(url)
            .header("User-Agent", "crates-pro")
            .send()
            .await;

        let res = match res {
            Ok(response) => {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response body".to_string());

                if status.is_success() {
                    // 解析 JSON
                    match serde_json::from_str::<GitHubSearchResponse>(&body) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            eprintln!("❌ JSON Parse Error: {:?}\nRaw Response: {}", e, body);
                            panic!("{}", e)
                        }
                    }
                } else {
                    eprintln!("❌ HTTP Error: {} - {}", status, body);
                    panic!("Invalid Response")
                }
            }
            Err(err) => {
                eprintln!("❌ Request failed: {:?}", err);
                panic!("{}", err)
            }
        };

        let items = res.items;
        if items.is_empty() {
            break;
        }

        let mut save_models = vec![];
        for item in items {
            println!("Repo {} | Clone URL: {}", item.name, item.clone_url);
            let model = programs::ActiveModel {
                id: Set(Uuid::new_v4()),
                github_url: Set(item.clone_url),
                name: Set(item.name),
                description: Set("".to_owned()),
                namespace: Set("".to_owned()),
                max_version: Set("".to_owned()),
                mega_url: Set("".to_owned()),
                doc_url: Set("".to_owned()),
                program_type: Set("".to_owned()),
                downloads: Set(0),
                cratesio: Set("".to_owned()),
            };
            save_models.push(model);
        }
        context
            .program_storage()
            .save_programs(save_models)
            .await
            .unwrap();
        println!("next page :{}", page);
        page += 1;
    }
    Ok(())
}

mod test {}
