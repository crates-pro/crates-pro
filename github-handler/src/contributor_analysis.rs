use chrono::{DateTime, FixedOffset};
use database::storage::Context;
use entity::{github_user, programs};
use futures::TryStreamExt;
use model::github::{AnalyzedUser, ContributorAnalysis};
use sea_orm::ActiveValue::Set;
use sea_orm::IntoActiveModel;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use tokio::process::Command as TokioCommand;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{contributor_analysis, services::github_api::GitHubApiClient, BoxError};
use crate::{git, utils};

// 中国相关时区
const CHINA_TIMEZONES: [&str; 4] = ["+0800", "+08:00", "CST", "Asia/Shanghai"];

/// 判断时区是否可能是中国时区
fn is_china_timezone(timezone: &str) -> bool {
    CHINA_TIMEZONES.iter().any(|&tz| timezone.contains(tz))
}

/// 分析贡献者的时区统计
pub async fn analyze_contributor_timezone(
    repo_path: &PathBuf,
    analyzed_emails: &HashSet<String>,
) -> Option<ContributorAnalysis> {
    if !repo_path.exists() {
        error!("仓库路径不存在: {}", repo_path.display());
        return None;
    }
    // 用于分析的邮箱可能存在多个不同的值，如profile 设置的值，commit时设置的值
    debug!("分析作者 {:?} 的时区统计", analyzed_emails);

    let mut commits = vec![];
    for email in analyzed_emails {
        // 获取提交时区分布
        match get_author_commits(repo_path, email).await {
            Some(result) => {
                if !result.is_empty() {
                    commits = result;
                    break;
                }
            }
            None => {
                continue;
                // warn!("无法获取作者提交: {}", author_email);
                // return None;
            }
        };
    }

    if commits.is_empty() {
        warn!("作者没有提交记录: {:?}", analyzed_emails);
        return None;
    }

    let mut has_china_timezone = false;
    let mut timezone_count: HashMap<String, usize> = HashMap::new();

    // 分析每个提交的时区
    // TODO: 是否有必要分析每个提交的时区，如果遇到一个就认为是中国，可能优化性能
    // 如果遇到一个就认为是中国，可能优化性能
    for commit in &commits {
        let timezone = &commit.timezone;

        // 更新时区统计
        *timezone_count.entry(timezone.clone()).or_insert(0) += 1;

        // 检查是否为中国时区
        if is_china_timezone(timezone) {
            has_china_timezone = true;
        }
    }

    // 找出最常用的时区
    let common_timezone = timezone_count
        .iter()
        .max_by_key(|(_, &count)| count)
        .map(|(tz, _)| tz.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let analysis = ContributorAnalysis {
        has_china_timezone,
        common_timezone,
    };

    Some(analysis)
}

#[derive(Debug)]
struct CommitInfo {
    _datetime: DateTime<FixedOffset>,
    timezone: String,
}

/// 从git log里面获取作者的所有提交
async fn get_author_commits(repo_path: &PathBuf, author_email: &str) -> Option<Vec<CommitInfo>> {
    let output = TokioCommand::new("git")
        .current_dir(repo_path)
        .args([
            "log",
            "--format=%aI", // ISO 8601 格式的作者日期
            "--author",
            author_email,
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect();

    let mut commits = Vec::new();

    for line in lines {
        if let Ok(dt) = line.parse::<DateTime<FixedOffset>>() {
            // 提取时区部分
            let timezone = if let Some(pos) = line.rfind(|c: char| ['+', '-'].contains(&c)) {
                line[pos..].to_string()
            } else if line.contains("Z") {
                "Z".to_string() // UTC
            } else {
                "Unknown".to_string()
            };

            commits.push(CommitInfo {
                _datetime: dt,
                timezone,
            });
        }
    }

    Some(commits)
}

// 分析贡献者国别位置
async fn analyze_contributor_locations(
    context: Context,
    owner: &str,
    repo: &str,
    repository_id: Uuid,
    analyzed_users: &[AnalyzedUser],
) -> Result<(), BoxError> {
    debug!("分析仓库 {}/{} 的贡献者地理位置", owner, repo);
    let base_dir = context.base_dir.clone();
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir)?;
        debug!("创建根目录: {:?}", base_dir);
    }

    let target_dir = utils::repo_dir(base_dir, owner, repo);

    // 检查目录是否已存在
    if !target_dir.exists() {
        // 确保父目录存在
        if let Some(parent) = target_dir.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        git::clone_repo(&target_dir, owner, repo, true).await?;
    } else if git::is_shallow_repo(&target_dir) {
        git::restore_shallow_repo(&target_dir).await?;
    } else {
        git::update_repo(&target_dir, owner, repo).await?;
    }

    debug!("开始分析 {} 个贡献者的时区信息", analyzed_users.len());

    let mut china_contributors = 0;
    let mut non_china_contributors = 0;

    // 对每个贡献者进行时区分析
    for user in analyzed_users.iter() {
        // 使用贡献者的邮箱进行时区分析
        if user.commit_email.is_none() && user.profile_email.is_none() {
            error!("用户 {} 没有邮箱信息", user.login);
            continue;
        }

        let mut analyzed_emails = HashSet::new();

        for email in [user.profile_email.as_ref(), user.commit_email.as_ref()]
            .into_iter()
            .flatten()
        {
            analyzed_emails.insert(email.clone());
        }
        // 分析该贡献者的时区情况
        let analysis =
            match contributor_analysis::analyze_contributor_timezone(&target_dir, &analyzed_emails)
                .await
            {
                Some(result) => result,
                None => {
                    warn!("无法分析用户 {} 的时区信息", user.login);
                    continue;
                }
            };

        // 存储贡献者位置分析
        if let Err(e) = context
            .github_handler_stg()
            .store_contributor_location(repository_id, user.user_id, &analysis)
            .await
        {
            error!("存储贡献者位置分析失败: {}", e);
        }

        // 统计中国贡献者和非中国贡献者
        if analysis.common_timezone == "+08:00" {
            china_contributors += 1;
            info!(
                "贡献者 {} 可能来自中国, 常用时区: {}",
                user.login, analysis.common_timezone
            );
        } else {
            non_china_contributors += 1;
            info!(
                "贡献者 {} 可能来自海外, 常用时区: {}",
                user.login, analysis.common_timezone
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
    // match context
    //     .github_handler_stg()
    //     .get_repository_china_contributor_stats(repository_id)
    //     .await
    // {
    //     Ok(stats) => {
    //         info!(
    //             "仓库 {}/{} 的中国贡献者统计: {}人中有{}人来自中国 ({:.1}%)",
    //             owner,
    //             repo,
    //             stats.total_contributors,
    //             stats.china_contributors,
    //             stats.china_percentage
    //         );

    //         if !stats.china_contributors_details.is_empty() {
    //             info!("中国贡献者TOP列表:");
    //             for (i, contributor) in stats.china_contributors_details.iter().enumerate().take(5)
    //             {
    //                 let name_display = contributor
    //                     .name
    //                     .clone()
    //                     .unwrap_or_else(|| contributor.login.clone());
    //                 info!(
    //                     "  {}. {} - {} 次提交",
    //                     i + 1,
    //                     name_display,
    //                     contributor.contributions
    //                 );
    //             }
    //         }
    //     }
    //     Err(e) => {
    //         error!("获取中国贡献者统计失败: {}", e);
    //     }
    // }

    Ok(())
}

// 分析Git贡献者
pub(crate) async fn analyze_git_contributors(
    context: Context,
    owner: &str,
    repo: &str,
) -> Result<(), BoxError> {
    debug!("分析仓库贡献者: {}/{}", owner, repo);

    // 创建GitHub API客户端
    let github_client = GitHubApiClient::new();

    // 获取仓库ID
    let repository_id = match context
        .github_handler_stg()
        .get_repository_id(owner, repo)
        .await?
    {
        Some(id) => id,
        None => {
            let repository = github_client.get_repo_info(owner, repo).await;
            match repository {
                Ok(repository) => {
                    let programs: programs::ActiveModel = repository.into();
                    let id = programs.clone().r#id.unwrap();
                    context
                        .github_handler_stg()
                        .save_or_update_programs_by_node_id(vec![programs])
                        .await
                        .unwrap();
                    id
                }
                Err(_) => {
                    warn!("无法查询到仓库 {}/{}", owner, repo);
                    return Ok(());
                }
            }
        }
    };

    // 获取仓库贡献者
    let contributors = github_client
        .get_all_repository_contributors(owner, repo)
        .await?;

    // 存储所有获取的用户信息，用于后续分析
    let mut analyzed_users: Vec<AnalyzedUser> = Vec::new();

    // 存储贡献者信息
    for contributor in &contributors {
        let user = match context
            .github_handler_stg()
            .get_user_by_name(&contributor.login)
            .await
            .unwrap()
        {
            Some(user) => user,
            None => {
                // 获取并存储用户详细信息
                let user = match github_client.get_user_details(&contributor.login).await {
                    Ok(user) => user,
                    Err(e) => {
                        warn!("获取用户 {} 详情失败: {}", contributor.login, e);
                        continue;
                    }
                };

                if user.is_bot() {
                    info!("skip bot:{}:", user.login);
                    continue;
                }

                let a_model: github_user::ActiveModel = user.into();
                // 存储用户到数据库
                context.github_handler_stg().store_user(a_model).await?
            }
        };

        // 从commit 获取email
        let commit_email = github_client
            .get_user_email_from_commits(owner, repo, &contributor.login)
            .await?;

        let mut a_user: AnalyzedUser = user.clone().into();
        a_user.commit_email = commit_email;
        // 保存用户信息用于后续分析
        analyzed_users.push(a_user);

        // 存储贡献者关系
        // if let Err(e) = context
        //     .github_handler_stg()
        //     .store_contributor(repository_id, user.id, contributor.contributions)
        //     .await
        // {
        //     error!(
        //         "存储贡献者关系失败: {}/{} -> {}: {}",
        //         owner, repo, user.login, e
        //     );
        // }
    }

    // 查询并显示贡献者统计
    // match context
    //     .github_handler_stg()
    //     .query_top_contributors(&repository_id)
    //     .await
    // {
    //     Ok(top_contributors) => {
    //         info!("仓库 {}/{} 的贡献者统计:", owner, repo);
    //         for (i, contributor) in top_contributors.iter().enumerate().take(10) {
    //             info!(
    //                 "  {}. {} - {} 次提交",
    //                 i + 1,
    //                 contributor.login,
    //                 contributor.contributions
    //             );
    //         }
    //     }
    //     Err(e) => {
    //         error!("查询贡献者统计失败: {}", e);
    //     }
    // }

    // 分析贡献者国别 - 传递已获取的用户信息
    analyze_contributor_locations(context, owner, repo, repository_id, &analyzed_users).await?;

    Ok(())
}

// 查询仓库的顶级贡献者
pub async fn query_top_contributors(
    context: Context,
    owner: &str,
    repo: &str,
) -> Result<(), BoxError> {
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
        .query_top_contributors(repository_id)
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
        }
        Err(e) => {
            error!("获取中国贡献者统计失败: {}", e);
        }
    }

    Ok(())
}

pub async fn analyze_all(
    context: Context,
    cratesio: bool,
    not_analyzed: bool,
) -> Result<(), BoxError> {
    let stg = context.github_handler_stg();
    let url_stream = stg.query_programs_stream(cratesio).await.unwrap();

    // 并发处理 Stream
    url_stream
        .try_for_each_concurrent(8, |model| {
            let context = context.clone();
            let stg = stg.clone();
            async move {
                // 通过数据库数据存在判断
                if not_analyzed {
                    let exist = stg.check_program_in_analyze(model.id).await?;
                    if !exist {
                        process_item(&model, context).await;
                    }
                } else if !model.github_analyzed {
                    // 通过flag判断
                    process_item(&model, context).await;
                }
                Ok(())
            }
        })
        .await?;
    Ok(())
}

pub async fn process_item(model: &programs::Model, context: Context) {
    if let Some((owner, repo)) = utils::parse_to_owner_and_repo(&model.github_url) {
        let res = analyze_git_contributors(context.clone(), &owner, &repo).await;
        if res.is_ok() {
            let mut a_model = model.clone().into_active_model();
            a_model.github_analyzed = Set(true);
            context
                .github_handler_stg()
                .update_program(a_model)
                .await
                .unwrap();
        }
    } else {
        tracing::error!("URL 格式不正确: {}", model.github_url);
    }
}
