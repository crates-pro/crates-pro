use std::path::PathBuf;

use clap::{Parser, Subcommand};
use contributor_analysis::analyze_git_contributors;
use database::storage::Context;
use entity::programs;
use futures::TryStreamExt;
use regex::Regex;
use sea_orm::{ActiveValue::Set, IntoActiveModel};
use tracing::{error, info, warn};

use crate::services::github_api::GitHubApiClient;

// 导入模块
mod config;
mod contributor_analysis;
// mod entities;
mod services;

// CLI 参数结构
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 仓库所有者（可选）
    owner: Option<String>,

    /// 仓库名称（可选）
    repo: Option<String>,

    /// 生成示例配置文件
    #[arg(long)]
    sample_config: Option<String>,

    /// 子命令
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 拉取GitHub仓库地址到数据库
    GithubSync,
    /// 分析所有拉取的仓库地址
    AnalyzeAll,
    /// 分析仓库贡献者
    Analyze {
        /// 仓库所有者
        owner: String,

        /// 仓库名称
        repo: String,
    },

    /// 查询仓库贡献者统计
    Query {
        /// 仓库所有者
        owner: String,

        /// 仓库名称
        repo: String,
    },
}

// 定义错误类型
type BoxError = Box<dyn std::error::Error + Send + Sync>;

// 初始化日志
fn init_logger() {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();
}

// 查询仓库的顶级贡献者
async fn query_top_contributors(context: Context, owner: &str, repo: &str) -> Result<(), BoxError> {
    info!("查询仓库 {}/{} 的顶级贡献者", owner, repo);

    // 获取仓库ID
    let repository_id = match context
        .github_handler_stg()
        .get_repository_id(owner, repo)
        .await?
    {
        Some(id) => id,
        None => {
            warn!("仓库 {}/{} 未在数据库中注册", owner, repo);
            return Ok(());
        }
    };

    // 查询贡献者统计
    match context
        .github_handler_stg()
        .query_top_contributors(&repository_id)
        .await
    {
        Ok(top_contributors) => {
            info!("仓库 {}/{} 的贡献者统计:", owner, repo);
            for (i, contributor) in top_contributors.iter().enumerate().take(10) {
                let location_str = contributor
                    .location
                    .as_ref()
                    .map(|loc| format!(" ({})", loc))
                    .unwrap_or_default();

                let name_display = contributor.name.as_ref().unwrap_or(&contributor.login);

                info!(
                    "  {}. {}{} - {} 次提交",
                    i + 1,
                    name_display,
                    location_str,
                    contributor.contributions
                );
            }
        }
        Err(e) => {
            error!("查询贡献者统计失败: {}", e);
        }
    }

    // 查询中国贡献者统计
    match context
        .github_handler_stg()
        .get_repository_china_contributor_stats(&repository_id)
        .await
    {
        Ok(stats) => {
            info!(
                "仓库 {}/{} 的中国贡献者统计: {}人中有{}人来自中国 ({:.1}%)",
                owner,
                repo,
                stats.total_contributors,
                stats.china_contributors,
                stats.china_percentage
            );
        }
        Err(e) => {
            error!("获取中国贡献者统计失败: {}", e);
        }
    }

    Ok(())
}

async fn sync_all_repos(context: Context) -> Result<(), BoxError> {
    let stg = context.github_handler_stg();
    let url_stream = stg.query_programs_stream().await.unwrap();

    // 并发处理 Stream
    url_stream
        .try_for_each_concurrent(8, |model| {
            let context = context.clone();
            async move {
                if !model.github_analyzed {
                    process_item(&model, context).await;
                }
                Ok(())
            }
        })
        .await?;
    Ok(())
}

async fn process_item(model: &programs::Model, context: Context) {
    let re = Regex::new(r"github\.com/([^/]+)/([^/]+)").unwrap();
    if let Some(captures) = re.captures(&model.github_url) {
        let owner = &captures[1];
        let repo = &captures[2];
        let res = analyze_git_contributors(context.clone(), owner, repo).await;
        if res.is_ok() {
            let mut a_model = model.clone().into_active_model();
            a_model.github_analyzed = Set(true);
            context.github_handler_stg().update_program(a_model).await.unwrap();
        }
    } else {
        tracing::error!("URL 格式不正确: {}", model.github_url);
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // 加载.env文件
    dotenvy::dotenv().ok();

    // 初始化日志
    init_logger();

    // 解析命令行参数
    let cli = Cli::parse();

    // 连接数据库
    info!("连接数据库...");
    let config = config::load_config().unwrap();
    let context = Context::new(
        &config.database.url,
        &config.github.tokens[0],
        PathBuf::from(config.repopath),
    )
    .await;

    // 处理子命令
    match cli.command {
        Some(Commands::Analyze { owner, repo }) => {
            analyze_git_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::Query { owner, repo }) => {
            query_top_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::GithubSync) => {
            let github_client = GitHubApiClient::new();
            github_client.start_graphql_sync(&context).await?;
        }

        Some(Commands::AnalyzeAll) => sync_all_repos(context).await?,

        None => {
            // 如果没有提供子命令，但提供了owner和repo参数
            if let (Some(owner), Some(repo)) = (cli.owner, cli.repo) {
                analyze_git_contributors(context, &owner, &repo).await?;
            } else {
                // 没有足够的参数，显示帮助信息
                println!("请提供仓库所有者和名称，或使用子命令。运行 --help 获取更多信息。");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use futures::stream::TryStreamExt;
    use futures::{stream, StreamExt};
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::time::sleep;
    use walkdir::WalkDir;

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

    #[test]
    fn move_histiry() {
        let config = crate::config::load_config().unwrap();
        let base_dir = PathBuf::from(config.repopath);
        for entry in WalkDir::new(&base_dir)
            .min_depth(1)
            .max_depth(2) // Only go two levels: source/owner/repo
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_dir())
        {
            let path = entry.path().to_path_buf();

            // We are interested in paths like source/owner/repo
            if path.parent() == Some(&base_dir) {
                // This is an owner directory: source/owner
                let owner_name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(name) => name,
                    None => continue,
                };

                if owner_name.len() < 4 {
                    println!("Skipping short owner: {}", owner_name);
                    continue;
                }

                let ow = &owner_name[0..2];
                let ne = &owner_name[2..4];
                let nested_path = base_dir.join(ow).join(ne);
                fs::create_dir_all(&nested_path).unwrap();

                for repo_entry in fs::read_dir(&path).unwrap() {
                    let repo = repo_entry.unwrap();
                    let repo_name = repo.file_name();
                    let new_path = nested_path.join(&repo_name);

                    if new_path.exists() {
                        println!("跳过：目标路径已存在: {}", new_path.display());
                    } else {
                        println!("Moving {} -> {}", repo.path().display(), new_path.display());
                        fs::rename(repo.path(), new_path).unwrap();
                    }
                }

                // Remove the old owner directory
                fs::remove_dir_all(path).unwrap();
            }
        }
    }
}
