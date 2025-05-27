use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};
use database::storage::Context;
use services::sync_repo;
use tracing::info;

use crate::services::github_api::GitHubApiClient;

// 导入模块
mod config;
mod contributor_analysis;
mod git;
mod services;
mod utils;

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
    SyncUrl,
    /// 单独拉取仓库
    SyncRepo {
        // 只拉取cratesio仓库
        #[arg(long, action = ArgAction::SetTrue)]
        cratesio: bool,
    },
    /// 分析所有拉取的仓库地址
    AnalyzeAll {
        #[arg(long, action = ArgAction::SetTrue)]
        cratesio: bool,
        // 只
        #[arg(long, action = ArgAction::SetTrue)]
        not_analyzed: bool,
    },
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
    /// 给crates 表和关联的programs 表设置nodeid
    UpdateCratesNodeid,
    /// 给programs设置 in_cratesio 字段
    UpdateProgram,
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
    let context = Context::new(&config.database.url, PathBuf::from(config.repopath)).await;

    // 处理子命令
    match cli.command {
        Some(Commands::Analyze { owner, repo }) => {
            contributor_analysis::analyze_git_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::Query { owner, repo }) => {
            contributor_analysis::query_top_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::SyncUrl) => {
            let github_client = GitHubApiClient::new();
            github_client.start_graphql_sync(&context).await?;
        }

        Some(Commands::AnalyzeAll { cratesio, not_analyzed }) => {
            tracing::info!("cratesio:{}, not_analyzed:{}", cratesio, not_analyzed);
            contributor_analysis::analyze_all(context, cratesio, not_analyzed).await?;
        }

        Some(Commands::UpdateCratesNodeid) => {
            sync_repo::update_crates_nodeid(context).await?;
        }

        Some(Commands::SyncRepo { cratesio }) => {
            sync_repo::sync_repo_with_sha(context, cratesio).await?;
        }

        Some(Commands::UpdateProgram) => {
            sync_repo::update_programs(context).await?;
        }

        None => {
            // 如果没有提供子命令，但提供了owner和repo参数
            if let (Some(owner), Some(repo)) = (cli.owner, cli.repo) {
                contributor_analysis::analyze_git_contributors(context, &owner, &repo).await?;
            } else {
                // 没有足够的参数，显示帮助信息
                println!("请提供仓库所有者和名称，或使用子命令。运行 --help 获取更多信息。");
            }
        }
    }

    Ok(())
}
