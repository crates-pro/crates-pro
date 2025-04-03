use clap::{Parser, Subcommand};
use database::storage::Context;
use model::github::{Contributor, GitHubUser};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tracing::{error, info, warn};

// 导入模块
mod config;
mod contributor_analysis;
// mod entities;
mod services;

use crate::config::get_database_url;
use crate::contributor_analysis::generate_contributors_report;
use crate::services::github_api::GitHubApiClient;

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

    /// 分析贡献者地理位置
    #[arg(long)]
    analyze_contributors: Option<String>,

    /// 子命令
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    SyncUrl,
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

// 分析Git贡献者
async fn analyze_git_contributors(
    context: Context,
    owner: &str,
    repo: &str,
) -> Result<(), BoxError> {
    info!("分析仓库贡献者: {}/{}", owner, repo);

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

    // 创建GitHub API客户端
    let github_client = GitHubApiClient::new();

    // 获取仓库贡献者
    let contributors = github_client
        .get_all_repository_contributors(owner, repo)
        .await?;

    info!("获取到 {} 个贡献者，开始存储到数据库", contributors.len());

    // 使用HashMap存储邮箱到用户ID的映射，用于后续分析
    let mut email_to_user_id = HashMap::new();
    // 存储所有获取的用户信息，用于后续分析
    let mut github_users = Vec::new();

    // 存储贡献者信息
    for contributor in &contributors {
        // 获取并存储用户详细信息
        let mut user = match github_client.get_user_details(&contributor.login).await {
            Ok(user) => user,
            Err(e) => {
                warn!("获取用户 {} 详情失败: {}", contributor.login, e);
                continue;
            }
        };

        // 如果API返回的用户没有邮箱但贡献信息中有，则使用贡献中的邮箱
        if user.email.is_none() && contributor.email.is_some() {
            user.email = contributor.email.clone();
        }

        // 存储用户到数据库
        let user_id = match context.github_handler_stg().store_user(&user).await {
            Ok(id) => id,
            Err(e) => {
                error!("存储用户 {} 失败: {}", user.login, e);
                continue;
            }
        };

        // 保存邮箱到用户ID的映射
        if let Some(email) = &user.email {
            email_to_user_id.insert(email.clone(), user_id);
            info!("记录邮箱映射: {} -> ID {}", email, user_id);
        }

        // 保存用户信息用于后续分析
        github_users.push(user.clone());

        // 存储贡献者关系
        if let Err(e) = context
            .github_handler_stg()
            .store_contributor(&repository_id, user_id, contributor.contributions)
            .await
        {
            error!(
                "存储贡献者关系失败: {}/{} -> {}: {}",
                owner, repo, user.login, e
            );
        }

        // 等待一小段时间，避免触发GitHub API限制
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // 查询并显示贡献者统计
    match context
        .github_handler_stg()
        .query_top_contributors(&repository_id)
        .await
    {
        Ok(top_contributors) => {
            info!("仓库 {}/{} 的贡献者统计:", owner, repo);
            for (i, contributor) in top_contributors.iter().enumerate().take(10) {
                info!(
                    "  {}. {} - {} 次提交",
                    i + 1,
                    contributor.login,
                    contributor.contributions
                );
            }
        }
        Err(e) => {
            error!("查询贡献者统计失败: {}", e);
        }
    }

    // 分析贡献者国别 - 传递已获取的用户信息
    analyze_contributor_locations(
        context,
        owner,
        repo,
        &repository_id,
        &contributors,
        &github_users,
        &email_to_user_id,
    )
    .await?;

    Ok(())
}

// 分析贡献者国别位置
async fn analyze_contributor_locations(
    context: Context,
    owner: &str,
    repo: &str,
    repository_id: &str,
    contributors: &[Contributor],
    github_users: &[GitHubUser],
    email_to_user_id: &HashMap<String, i32>,
) -> Result<(), BoxError> {
    info!("分析仓库 {}/{} 的贡献者地理位置", owner, repo);

    let base_dir = Path::new("/Users/Yetianxing/github_source");
    if !base_dir.exists() {
        fs::create_dir_all(base_dir)?;
        info!("创建根目录: {:?}", base_dir);
    }

    // 构建目标路径: /mnt/crates/github_source/{owner}/{repo}
    let target_dir = base_dir.join(format!("{}/{}", owner, repo));
    let target_path = target_dir.to_string_lossy();

    // 检查目录是否已存在
    if !target_dir.exists() {
        // 确保父目录存在
        if let Some(parent) = target_dir.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        info!("克隆仓库到指定目录: {}", target_path);
        let status = Command::new("git")
            .args([
                "clone",
                &format!("https://github.com/{}/{}.git", owner, repo),
                &target_path,
            ])
            .status();

        match status {
            Ok(status) if !status.success() => {
                warn!("克隆仓库失败: {}", status);
                return Ok(());
            }
            Err(e) => {
                warn!("执行git命令失败: {}", e);
                return Ok(());
            }
            _ => {}
        }
    } else {
        info!("更新已存在的仓库: {}", target_path);
        let status = Command::new("git")
            .current_dir(&target_dir)
            .args(["pull"])
            .status();

        if let Err(e) = status {
            warn!("更新仓库失败: {}", e);
        }
    }

    info!("开始分析 {} 个贡献者的时区信息", github_users.len());

    let mut china_contributors = 0;
    let mut non_china_contributors = 0;

    // 对每个贡献者进行时区分析
    for user in github_users.iter() {
        // 使用贡献者的邮箱进行时区分析
        let email = match &user.email {
            Some(email) => email.clone(),
            None => {
                // 查找对应的contributor是否有邮箱
                let contributor_email = contributors
                    .iter()
                    .find(|c| c.login == user.login)
                    .and_then(|c| c.email.clone());

                match contributor_email {
                    Some(email) => email,
                    None => {
                        warn!("用户 {} 没有邮箱信息，使用登录名作为替代", user.login);
                        format!("{}@github.com", user.login)
                    }
                }
            }
        };

        // 分析该贡献者的时区情况
        let analysis = match contributor_analysis::analyze_contributor_timezone(
            target_path.as_ref(),
            &email,
        )
        .await
        {
            Some(result) => result,
            None => {
                warn!("无法分析用户 {} 的时区信息", user.login);
                continue;
            }
        };

        // 查找用户ID
        let user_id = match email_to_user_id.get(&email) {
            Some(id) => *id,
            None => match context
                .github_handler_stg()
                .get_user_id_by_name(&user.login)
                .await
            {
                Ok(Some(id)) => id,
                _ => {
                    warn!("未找到用户 {} 的ID", user.login);
                    continue;
                }
            },
        };

        // 存储贡献者位置分析
        if let Err(e) = context
            .github_handler_stg()
            .store_contributor_location(repository_id, user_id, &analysis)
            .await
        {
            error!("存储贡献者位置分析失败: {}", e);
        }

        // 统计中国贡献者和非中国贡献者
        if analysis.from_china {
            china_contributors += 1;
            info!(
                "贡献者 {} (邮箱: {}) 可能来自中国, 常用时区: {}",
                user.login, email, analysis.common_timezone
            );
        } else {
            non_china_contributors += 1;
            info!(
                "贡献者 {} (邮箱: {}) 可能来自海外, 常用时区: {}",
                user.login, email, analysis.common_timezone
            );
        }
    }

    let total_contributors = china_contributors + non_china_contributors;
    let china_percentage = if total_contributors > 0 {
        (china_contributors as f64 / total_contributors as f64) * 100.0
    } else {
        0.0
    };

    info!(
        "时区分析完成: 总计 {} 位贡献者, 其中中国贡献者 {} 位 ({:.1}%), 海外贡献者 {} 位 ({:.1}%)",
        total_contributors,
        china_contributors,
        china_percentage,
        non_china_contributors,
        100.0 - china_percentage
    );

    // 查询中国贡献者统计
    match context
        .github_handler_stg()
        .get_repository_china_contributor_stats(repository_id)
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

            if !stats.china_contributors_details.is_empty() {
                info!("中国贡献者TOP列表:");
                for (i, contributor) in stats.china_contributors_details.iter().enumerate().take(5)
                {
                    let name_display = contributor
                        .name
                        .clone()
                        .unwrap_or_else(|| contributor.login.clone());
                    info!(
                        "  {}. {} - {} 次提交",
                        i + 1,
                        name_display,
                        contributor.contributions
                    );
                }
            }
        }
        Err(e) => {
            error!("获取中国贡献者统计失败: {}", e);
        }
    }

    Ok(())
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

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // 加载.env文件
    dotenvy::dotenv().ok();

    // 初始化日志
    init_logger();

    // 解析命令行参数
    let cli = Cli::parse();

    // 处理贡献者分析请求
    if let Some(repo_path) = cli.analyze_contributors {
        let report = generate_contributors_report(&repo_path).await;
        report.print_summary();

        // 如果提供了第二个位置参数，将结果保存为JSON
        if let Some(output_path) = cli.repo {
            let json = report.to_json()?;
            std::fs::write(&output_path, json)?;
            info!("分析结果已保存到: {}", output_path);
        }

        return Ok(());
    }

    // 连接数据库
    info!("连接数据库...");
    let db_url = get_database_url();
    let context = Context::new(&db_url).await;

    // 处理子命令
    match cli.command {
        Some(Commands::Analyze { owner, repo }) => {
            analyze_git_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::Query { owner, repo }) => {
            query_top_contributors(context, &owner, &repo).await?;
        }

        Some(Commands::SyncUrl{}) => {
            let github_client = GitHubApiClient::new();
            github_client.start_graphql_sync(&context).await?;
        }

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
