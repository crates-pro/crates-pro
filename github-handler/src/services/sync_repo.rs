use crate::{git, utils, BoxError};
use database::storage::Context;
use entity::programs;
use futures::{StreamExt, TryStreamExt};
use sea_orm::{ActiveValue::Set, IntoActiveModel};
use tracing::error;

use super::github_api::GitHubApiClient;

pub(crate) async fn sync_repo_with_sha(
    context: Context,
    cratesio: bool,
) -> Result<(), anyhow::Error> {
    let stg = context.github_handler_stg();
    let url_stream = stg.query_programs_stream(cratesio).await.unwrap();

    url_stream
        .try_for_each_concurrent(16, |model| {
            let context = context.clone();
            let base_dir = context.base_dir.clone();

            async move {
                if let Some((owner, repo)) = utils::parse_to_owner_and_repo(&model.github_url) {
                    let nested_path = utils::repo_dir(base_dir, &owner, &repo);
                    if nested_path.exists() {
                        git::update_repo(&nested_path, &owner, &repo).await.unwrap();
                    } else {
                        git::clone_repo(&nested_path, &owner, &repo, false)
                            .await
                            .unwrap();
                    }
                }
                Ok(())
            }
        })
        .await?;
    Ok(())
}

pub(crate) async fn update_programs(context: Context) -> Result<(), BoxError> {
    let stg = context.github_handler_stg();
    let mut crates_stream = stg.query_valid_crates().await.unwrap();
    let mut nodeids = Vec::new();
    while let Some(row) = crates_stream.next().await {
        let row = row?;
        nodeids.push(row.github_node_id.unwrap());
    }
    for chunks in nodeids.chunks(1000) {
        stg.update_programs_by_node_id(chunks.to_vec()).await?;
    }
    Ok(())
}

pub(crate) async fn update_crates_nodeid(context: Context) -> Result<(), BoxError> {
    let stg = context.github_handler_stg();
    let crates_stream = stg.query_crates_stream().await.unwrap();
    crates_stream
        .try_for_each_concurrent(16, |model| {
            let context = context.clone();
            async move {
                let repository = model.repository.clone().unwrap();
                if let Some((owner, repo)) = utils::parse_to_owner_and_repo(&repository) {
                    let github_client = GitHubApiClient::new();
                    let repo = github_client.get_repo_info(&owner, &repo).await;
                    let mut a_model = model.into_active_model();
                    match repo {
                        Ok(repo) => {
                            a_model.github_node_id = Set(Some(repo.node_id.clone()));
                            let mut programs: programs::ActiveModel = repo.into();
                            programs.in_cratesio = Set(true);
                            context
                                .github_handler_stg()
                                .save_or_update_programs_by_node_id(vec![programs])
                                .await
                                .unwrap()
                        }
                        Err(err) => {
                            if err.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                                error!("Repo Not Found, tag to invalid:{}", err);
                                a_model.repo_invalid = Set(true);
                            }
                        }
                    }
                    context
                        .github_handler_stg()
                        .update_crates(a_model)
                        .await
                        .unwrap();
                } else {
                    tracing::error!("URL 格式不正确: {}", &repository);
                }
                Ok(())
            }
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod test {
    use futures::stream::TryStreamExt;
    use futures::{stream, StreamExt};
    use reqwest::StatusCode;
    use std::error::Error;
    use std::time::Duration;
    use tokio::time::sleep;

    use crate::services::github_api::GitHubApiClient;

    #[tokio::test]
    async fn test_stream_concurrent() {
        let data = [1, 2, 3, 4, 5, 6, 7];
        stream::iter(data)
            .map(Ok::<i32, Box<dyn Error>>)
            .try_for_each_concurrent(3, |item| async move {
                println!("Start: {}", item);
                sleep(Duration::from_millis(1000)).await;
                println!("End: {}", item);
                Ok(())
            })
            .await
            .unwrap();
        println!("All done");
    }

    #[tokio::test]
    async fn test_api_404() {
        let github_client = GitHubApiClient::new();
        let res = github_client.get_repo_info("fake", "nothing").await;
        match res {
            Ok(_) => {}
            Err(e) => {
                if e.status() == Some(StatusCode::NOT_FOUND) {
                    println!("err: {}", e)
                } else {
                    panic!("Not 404")
                }
            }
        }
    }
}
