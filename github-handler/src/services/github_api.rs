use chrono::{DateTime, Duration, NaiveDate, Utc};
use database::storage::Context;
use entity::{github_sync_status, programs};
use futures::{stream, StreamExt};
use model::github::{Contributor, GitHubUser, RestfulRepository};
use reqwest::{header, Client, Error, Response};
use sea_orm::{
    prelude::Uuid,
    ActiveValue::{NotSet, Set},
};
use serde_json::json;
use tracing::{debug, error, info, warn};

// GitHub API URL
const GITHUB_API_URL: &str = "https://api.github.com";

// 使用main中定义的函数获取GitHub令牌
use crate::config::get_github_token;
use model::github::{CommitData, GraphQLResponse, Repository};

// GitHub API客户端
pub struct GitHubApiClient {
    client: Client,
}

impl GitHubApiClient {
    // 创建新的GitHub API客户端
    pub fn new() -> Self {
        // 初始化为不带认证的Client
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("github-handler")
            .build()
            .unwrap_or_else(|_| Client::new());

        GitHubApiClient { client }
    }

    // 创建带有认证头的请求构建器
    async fn authorized_request(&self, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        let token = get_github_token().await;
        let mut builder = self.client.get(url);

        if !token.is_empty() {
            builder = builder.header(header::AUTHORIZATION, format!("token {}", token));
        }

        let request = builder.header(header::USER_AGENT, "github-handler");

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!("API请求 {} 失败: {}", url, e);
                return Err(e);
            }
        };
        let response = self.github_api_limit_check(response, &token).await?;

        Ok(response)
    }

    // api 限流检查
    pub async fn github_api_limit_check(
        &self,
        response: Response,
        token: &str,
    ) -> Result<Response, reqwest::Error> {
        if !response.status().is_success() {
            // 如果是速率限制，打印详细信息
            if response.status() == reqwest::StatusCode::FORBIDDEN {
                if let Some(remain) = response.headers().get("x-ratelimit-remaining") {
                    error!(
                        "GitHub API速率限制剩余: {}",
                        remain.to_str().unwrap_or("未知")
                    );
                }
                if let Some(reset) = response.headers().get("x-ratelimit-reset") {
                    let reset_time = reset.to_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let wait_time = reset_time - now;
                    error!(
                        "GitHub API速率限制重置时间: {} (还需等待约{}秒), token: {}",
                        reset_time,
                        if wait_time > 0 { wait_time } else { 0 },
                        self.mask_token(token)
                    );
                }
                let remaining = response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(-1);

                if remaining == 0 {
                    // 标记令牌为已用完
                    crate::config::mark_token_exhausted(token.to_string()).await;
                    error!("GitHub API令牌已用完");
                }
            }
            return response.error_for_status();
        }
        Ok(response)
    }

    pub async fn verify_token(&self, token: &str) -> bool {
        let url = format!("{}/rate_limit", GITHUB_API_URL);
        let client = &self.client;

        let response = client
            .get(&url)
            .header(header::AUTHORIZATION, format!("token {}", token))
            .header(header::USER_AGENT, "github-handler")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Some(remaining) = resp
                        .headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<i32>().ok())
                    {
                        return remaining > 0;
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    fn mask_token(&self, token: &str) -> String {
        let len = token.len();
        if len <= 8 {
            "*".repeat(len)
        } else {
            let start = &token[..10];
            let end = &token[len - 4..];
            format!("{}{}{}", start, "*".repeat(len - 8), end)
        }
    }

    pub async fn get_repo_info(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<RestfulRepository, reqwest::Error> {
        let url = format!("{}/repos/{}/{}", GITHUB_API_URL, owner, repo);
        let response = self.authorized_request(&url).await?.error_for_status()?;
        let res: RestfulRepository = response.json().await?;
        tracing::info!("请求repo 信息成功:{:?}", res);
        Ok(res)
    }

    // 获取GitHub用户详细信息
    pub async fn get_user_details(&self, username: &str) -> Result<GitHubUser, anyhow::Error> {
        let url = format!("{}/users/{}", GITHUB_API_URL, username);
        debug!("请求用户信息: {}", url);

        let response = self.authorized_request(&url).await?.error_for_status()?;
        let user: GitHubUser = response.json().await?;

        Ok(user)
    }

    pub async fn get_all_repository_contributors(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Contributor>, anyhow::Error> {
        debug!("通过Contributor API获取所有仓库贡献者: {}/{}", owner, repo);

        // 使用HashMap统计每个贡献者的提交次数
        let mut contributors = vec![];
        let mut page = 1;
        let per_page = 100; // GitHub允许的最大值

        let max_pages = 100;

        while page <= max_pages {
            let url = format!(
                "{}/repos/{}/{}/contributors?page={}&per_page={}",
                GITHUB_API_URL, owner, repo, page, per_page
            );

            debug!("请求Contributor API: {} (第{}页)", url, page);

            let response = self.authorized_request(&url).await?;

            // 提取分页信息
            let has_next_page = response
                .headers()
                .get("link")
                .and_then(|h| h.to_str().ok())
                .map(|link| link.contains("rel=\"next\""))
                .unwrap_or(false);

            let mut page_contributors: Vec<Contributor> = match response.json().await {
                Ok(c) => c,
                Err(e) => {
                    warn!("解析提交数据失败: {}", e);
                    break;
                }
            };

            if page_contributors.is_empty() {
                debug!("没有更多提交数据");
                break;
            }

            contributors.append(&mut page_contributors);

            // 如果没有下一页，退出循环
            if !has_next_page {
                break;
            }
            page += 1;
        }

        info!("通过Contributors API找到 {} 名贡献者", contributors.len());

        Ok(contributors)
    }

    pub async fn get_user_email_from_commits(
        &self,
        owner: &str,
        repo: &str,
        login: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        let url = format!(
            "{}/repos/{}/{}/commits?page=1&per_page=1&author={}",
            GITHUB_API_URL, owner, repo, login
        );

        let response = self.authorized_request(&url).await?;

        let commits: Vec<CommitData> = response.json().await.map_err(|e| {
            error!("解析提交数据失败: {}", e);
            anyhow::anyhow!("解析提交数据失败: {}", e)
        })?;

        if commits.is_empty() {
            error!("无法根据 login 获取commit 信息:{}", url);
            return Ok(None);
        }
        let email = commits
            .first()
            .unwrap()
            .commit
            .author
            .as_ref()
            .and_then(|a| a.email.clone())
            .unwrap();
        Ok(Some(email))
    }

    // 获取所有仓库贡献者（通过Commits API）
    #[allow(dead_code)]
    pub async fn get_all_repository_contributors_by_commits(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Contributor>, Box<dyn std::error::Error + Send + Sync>> {
        info!("通过Commits API获取所有仓库贡献者: {}/{}", owner, repo);

        // 使用HashMap统计每个贡献者的提交次数
        let mut contributors_map = std::collections::HashMap::new();
        let mut page = 1;
        let per_page = 100; // GitHub允许的最大值

        // 获取最近10,000个提交（100页，每页100个）
        let max_pages = 100;

        while page <= max_pages {
            let url = format!(
                "{}/repos/{}/{}/commits?page={}&per_page={}",
                GITHUB_API_URL, owner, repo, page, per_page
            );

            debug!("请求Commits API: {} (第{}页)", url, page);

            let response = self.authorized_request(&url).await?;

            // 检查状态码
            if !response.status().is_success() {
                error!("获取提交页面 {} 失败: HTTP {}", page, response.status());
                // 如果是速率限制，打印详细信息
                if response.status() == reqwest::StatusCode::FORBIDDEN {
                    if let Some(remain) = response.headers().get("x-ratelimit-remaining") {
                        error!(
                            "GitHub API速率限制剩余: {}",
                            remain.to_str().unwrap_or("未知")
                        );
                    }
                    if let Some(reset) = response.headers().get("x-ratelimit-reset") {
                        let reset_time = reset.to_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let wait_time = reset_time - now;
                        error!(
                            "GitHub API速率限制重置时间: {} (还需等待约{}秒)",
                            reset_time,
                            if wait_time > 0 { wait_time } else { 0 }
                        );
                    }
                }
                break;
            }

            // 提取分页信息
            let has_next_page = response
                .headers()
                .get("link")
                .and_then(|h| h.to_str().ok())
                .map(|link| link.contains("rel=\"next\""))
                .unwrap_or(false);

            let commits: Vec<CommitData> = match response.json().await {
                Ok(c) => c,
                Err(e) => {
                    warn!("解析提交数据失败: {}", e);
                    break;
                }
            };

            if commits.is_empty() {
                info!("没有更多提交数据");
                break;
            }

            // 统计贡献者信息
            for commit in commits {
                // 获取提交中的电子邮箱
                let email = commit.commit.author.as_ref().and_then(|a| a.email.clone());

                if let Some(author) = commit.author {
                    contributors_map
                        .entry(author.id)
                        .and_modify(|e: &mut (String, String, i32, Option<String>)| {
                            e.2 += 1;
                            // 如果之前没有邮箱但现在有了，则更新
                            if e.3.is_none() && email.is_some() {
                                e.3 = email.clone();
                            }
                        })
                        .or_insert((author.login, author.avatar_url, 1, email));
                }
            }

            info!(
                "已处理 {} 页提交，当前贡献者数量: {}",
                page,
                contributors_map.len()
            );

            // 如果没有下一页，退出循环
            if !has_next_page {
                break;
            }

            // 添加延迟避免触发GitHub API限制
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            page += 1;
        }

        info!("通过Commits API找到 {} 名贡献者", contributors_map.len());

        // 转换为Contributor结构
        let mut commit_contributors = contributors_map
            .into_iter()
            .map(
                |(id, (login, avatar_url, contributions, email))| Contributor {
                    id,
                    login,
                    avatar_url,
                    contributions,
                    email,
                },
            )
            .collect::<Vec<_>>();

        // 按贡献数量排序
        commit_contributors.sort_by(|a, b| b.contributions.cmp(&a.contributions));

        Ok(commit_contributors)
    }

    pub async fn start_graphql_sync(&self, context: &Context) -> Result<(), Error> {
        let date = NaiveDate::parse_from_str("2010-06-16", "%Y-%m-%d").unwrap();
        let end_date = NaiveDate::parse_from_str("2025-05-23", "%Y-%m-%d").unwrap();
        // let threshold_date = NaiveDate::parse_from_str("2015-01-01", "%Y-%m-%d").unwrap();

        let dates: Vec<NaiveDate> = {
            let mut d = date;
            let mut v = Vec::new();
            while d <= end_date {
                v.push(d);
                d += Duration::days(1);
            }
            v
        };

        stream::iter(dates)
            .for_each_concurrent(4, |date| {
                let context = context.clone();
                async move {
                    tracing::info!("Syncing date: {}", date.format("%Y-%m-%d"));
                    if let Err(err) = self
                        .sync_with_date(
                            &context,
                            &date.format("%Y-%m-%d").to_string(),
                            &date.format("%Y-%m-%d").to_string(),
                        )
                        .await
                    {
                        tracing::error!("Failed to sync {}: {:?}", date, err);
                    }
                }
            })
            .await;

        Ok(())
    }

    async fn sync_with_date(
        &self,
        context: &Context,
        start_date: &str,
        end_date: &str,
    ) -> Result<(), Error> {
        let sync_record = context
            .github_handler_stg()
            .get_github_sync_status_by_date(start_date, end_date)
            .await
            .unwrap();
        if let Some(record) = sync_record {
            if record.sync_result {
                return Ok(());
            }
        }
        const GITHUB_API_URL: &str = "https://api.github.com/graphql";

        let client = reqwest::Client::new();
        let mut cursor: Option<String> = None;

        let page_success = loop {
            let query = r#"
        query ($query: String!, $cursor: String) {
            search(query: $query, type: REPOSITORY, first: 100, after: $cursor) {
                edges {
                    node {
                        ... on Repository {
                            id
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
            let token = get_github_token().await;
            let response = client
                .post(GITHUB_API_URL)
                .header("Authorization", format!("token {}", &token))
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
                    tracing::info!("response body:{}", body);
                    if status.is_success() {
                        match serde_json::from_str::<GraphQLResponse>(&body) {
                            Ok(parsed) => Some(parsed),
                            Err(e) => {
                                tracing::error!(
                                    "❌ JSON Parse Error: {:?}\nRaw Response: {}",
                                    e,
                                    body
                                );
                                None
                            }
                        }
                    } else {
                        tracing::error!(
                            "❌ HTTP Error: {} - {}, token: {}",
                            status,
                            body,
                            self.mask_token(&token)
                        );
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
                            .github_handler_stg()
                            .save_or_update_programs(save_models)
                            .await
                            .unwrap();
                        if data.search.page_info.has_next_page {
                            cursor = data.search.page_info.end_cursor;
                        } else {
                            // 没有下一页 正常退出
                            break true;
                        }
                    }
                    None => break false,
                }
            } else {
                break false;
            }
        };
        if page_success {
            context
                .github_handler_stg()
                .save_github_sync_status(github_sync_status::ActiveModel {
                    id: NotSet,
                    start_date: Set(start_date.to_owned()),
                    end_date: Set(end_date.to_owned()),
                    sync_result: Set(true),
                })
                .await
                .unwrap();
        }
        Ok(())
    }
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
        repo_created_at: Set(Some(
            item.created_at
                .parse::<DateTime<Utc>>()
                .unwrap()
                .naive_utc(),
        )),
        github_analyzed: Set(false),
        in_cratesio: Set(false),
        github_node_id: Set(item.id),
        updated_at: Set(Some(chrono::Utc::now().naive_utc())),
        repo_sync_at: Set(None),
    };
    save_models.push(model);
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn main() {
        let time_str = "2024-11-30T01:55:00Z";

        // 先解析成 DateTime<Utc>
        let datetime_utc: DateTime<Utc> = time_str.parse().expect("解析失败");

        // 然后转换为 NaiveDateTime（去掉时区信息）
        let naive: NaiveDateTime = datetime_utc.naive_utc();

        println!("NaiveDateTime: {}", naive);
    }
}
