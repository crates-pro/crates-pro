use std::env;

use chrono::{Duration, NaiveDate};
use database::storage::Context;
use entity::{github_sync_status, programs};
use model::{GitHubSearchResponse, GraphQLResponse, Repository};
use reqwest::Error;
use sea_orm::{
    prelude::Uuid,
    ActiveValue::{NotSet, Set},
};
use serde_json::json;

pub mod model;

pub async fn start_graphql_sync(context: &Context) -> Result<(), Error> {
    let mut date = NaiveDate::parse_from_str("2011-01-01", "%Y-%m-%d").unwrap();
    let end_date = NaiveDate::parse_from_str("2025-04-01", "%Y-%m-%d").unwrap();
    let threshold_date = NaiveDate::parse_from_str("2015-01-01", "%Y-%m-%d").unwrap();

    // let mut date = start_date;
    while date <= end_date {
        let next_date = if date < threshold_date {
            date + Duration::days(60)
        } else {
            date + Duration::days(1)
        };

        tracing::info!(
            "Syncing date: {} to {}",
            date.format("%Y-%m-%d"),
            next_date.format("%Y-%m-%d")
        );

        sync_with_date(
            context,
            &date.format("%Y-%m-%d").to_string(),
            &next_date.format("%Y-%m-%d").to_string(),
        )
        .await?;
        date = next_date;
    }

    Ok(())
}

async fn sync_with_date(context: &Context, start_date: &str, end_date: &str) -> Result<(), Error> {
    let sync_record = context
        .program_storage()
        .get_github_sync_status_by_date(start_date, end_date)
        .await
        .unwrap();
    if let Some(record) = sync_record {
        if record.sync_result {
            return Ok(());
        }
    }
    const GITHUB_API_URL: &str = "https://api.github.com/graphql";
    let github_token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN Not Set");

    let client = reqwest::Client::new();
    let mut cursor: Option<String> = None;

    loop {
        let query = r#"
        query ($query: String!, $cursor: String) {
            search(query: $query, type: REPOSITORY, first: 100, after: $cursor) {
                edges {
                    node {
                        ... on Repository {
                            name
                            url
                            createdAt
                        }
                    }
                }
                pageInfo {
                    endCursor
                    hasNextPage
                }
            }
        }
        "#;

        let variables = json!({ "query": format!("language:rust created:{}..{}", start_date, end_date), "cursor": cursor });

        let request_body = json!({
            "query": query,
            "variables": variables
        });

        let response = client
            .post(GITHUB_API_URL)
            .header("Authorization", github_token.to_string())
            .header("User-Agent", "Rust-GraphQL-Client")
            .json(&request_body)
            .send()
            .await;
        let res = match response {
            Ok(response) => {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read response body".to_string());
                tracing::info!("body : {:?}", body);
                if status.is_success() {
                    match serde_json::from_str::<GraphQLResponse>(&body) {
                        Ok(parsed) => Some(parsed),
                        Err(e) => {
                            tracing::error!("❌ JSON Parse Error: {:?}\nRaw Response: {}", e, body);
                            None
                        }
                    }
                } else {
                    tracing::error!("❌ HTTP Error: {} - {}", status, body);
                    None
                }
            }
            Err(err) => {
                tracing::error!("❌ Request failed: {:?}", err);
                None
            }
        };

        let mut save_models = vec![];

        if let Some(json) = res {
            match json.data {
                Some(data) => {
                    for edge in data.search.edges {
                        convert_to_model(edge.node, &mut save_models).await;
                    }
                    context
                        .program_storage()
                        .save_programs(save_models)
                        .await
                        .unwrap();
                    if data.search.page_info.has_next_page {
                        cursor = data.search.page_info.end_cursor;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        } else {
            break;
        }
    }
    context
        .program_storage()
        .save_github_sync_status(github_sync_status::ActiveModel {
            id: NotSet,
            start_date: Set(start_date.to_owned()),
            end_date: Set(end_date.to_owned()),
            sync_result: Set(true),
        })
        .await
        .unwrap();
    Ok(())
}

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
                            tracing::error!("❌ JSON Parse Error: {:?}\nRaw Response: {}", e, body);
                            panic!("{}", e)
                        }
                    }
                } else {
                    tracing::error!("❌ HTTP Error: {} - {}", status, body);
                    panic!("Invalid Response")
                }
            }
            Err(err) => {
                tracing::error!("❌ Request failed: {:?}", err);
                panic!("{}", err)
            }
        };

        let items = res.items;
        if items.is_empty() {
            break;
        }

        let mut save_models = vec![];
        for item in items {
            tracing::debug!("Repo {} | Clone URL: {}", item.name, item.url);
            convert_to_model(item, &mut save_models).await;
        }
        context
            .program_storage()
            .save_programs(save_models)
            .await
            .unwrap();
        tracing::debug!("next page :{}", page);
        page += 1;
    }
    Ok(())
}

async fn convert_to_model(item: Repository, save_models: &mut Vec<programs::ActiveModel>) {
    let model = programs::ActiveModel {
        id: Set(Uuid::new_v4()),
        github_url: Set(item.url),
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

mod test {}
