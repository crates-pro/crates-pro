use chrono::{DateTime, FixedOffset};
use database::storage::Context;
use model::github::{Contributor, ContributorAnalysis, GitHubUser};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tokio::process::Command as TokioCommand;
use tracing::{debug, error, info, warn};

use crate::{contributor_analysis, services::github_api::GitHubApiClient, BoxError};

// 中国相关时区
const CHINA_TIMEZONES: [&str; 4] = ["+0800", "+08:00", "CST", "Asia/Shanghai"];

/// 判断时区是否可能是中国时区
fn is_china_timezone(timezone: &str) -> bool {
    CHINA_TIMEZONES.iter().any(|&tz| timezone.contains(tz))
}

/// 分析贡献者的时区统计
pub async fn analyze_contributor_timezone(
    repo_path: &str,
    author_email: &str,
) -> Option<ContributorAnalysis> {
    if !Path::new(repo_path).exists() {
        error!("仓库路径不存在: {}", repo_path);
        return None;
    }

    debug!("分析作者 {} 的时区统计", author_email);

    // 获取提交时区分布
    let commits = match get_author_commits(repo_path, author_email).await {
        Some(commits) => commits,
        None => {
            warn!("无法获取作者提交: {}", author_email);
            return None;
        }
    };

    if commits.is_empty() {
        warn!("作者没有提交记录: {}", author_email);
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
        email: Some(author_email.to_string()),
        from_china: has_china_timezone,
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
async fn get_author_commits(repo_path: &str, author_email: &str) -> Option<Vec<CommitInfo>> {
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
    repository_id: &str,
    contributors: &[Contributor],
    github_users: &[GitHubUser],
    email_to_user_id: &HashMap<String, i32>,
) -> Result<(), BoxError> {
    info!("分析仓库 {}/{} 的贡献者地理位置", owner, repo);
    let base_dir = context.base_dir.clone();
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir)?;
        info!("创建根目录: {:?}", base_dir);
    }

    // 构建目标路径: /mnt/crates/github_source/{owner}/{repo}
    let target_dir = if owner.len() < 4 {
        base_dir.join(format!("{}/{}", owner, repo))
    } else {
        base_dir.join(format!("{}/{}/{}", &owner[..2], &owner[2..4], repo))
    };
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
                "--filter=blob:none", // 只clone 提交历史
                "--no-checkout",
                "--config",
                "credential.helper=reject", // 拒绝认证请求，不会提示输入
                "--config",
                "http.lowSpeedLimit=1000", // 设置低速限制
                "--config",
                "http.lowSpeedTime=10", // 如果速度低于限制持续10秒则失败
                "--config",
                "core.askpass=echo", // 不使用交互式密码提示
                &format!("https://github.com/{}/{}.git", owner, repo),
                &target_path,
            ])
            .status();

        match status {
            Ok(status) if !status.success() => {
                warn!("克隆仓库失败: {}，可能需要认证或不存在，跳过此仓库", status);
                return Ok(());
            }
            Err(e) => {
                warn!("执行git命令失败: {}，跳过此仓库", e);
                return Ok(());
            }
            _ => {}
        }
    } else if is_shallow_repo(&target_dir) {
        info!("更新之前clone的shallow仓库: {}", target_path);

        let args = vec![
            "-c",
            "credential.helper=reject",
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=10",
            "-c",
            "core.askpass=echo",
            "fetch",
            "--filter=blob:none", // 只clone 提交历史
            "--unshallow",
        ];
        let status = Command::new("git")
            .current_dir(&target_dir)
            .args(args)
            .status();
        if let Err(e) = status {
            warn!("更新仓库失败: {}，可能需要认证，继续分析当前代码", e);
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
        let analysis =
            match contributor_analysis::analyze_contributor_timezone(target_path.as_ref(), &email)
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
                .get_user_by_name(&user.login)
                .await
            {
                Ok(Some(model)) => model.id,
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

    // 查询是否存在贡献者位置信息
    // 用于判断是否需要重新分析
    // match context
    //     .github_handler_stg()
    //     .has_contributor_location(&repository_id)
    //     .await
    // {
    //     Ok(true) => {
    //         info!("仓库 {}/{} 已存在贡献者位置信息，跳过所有操作", owner, repo);
    //         return Ok(());
    //     }
    //     Ok(false) => {
    //         info!("仓库 {}/{} 没有贡献者位置信息，开始分析", owner, repo);
    //     }
    //     Err(e) => {
    //         error!("查询仓库 {}/{} 的贡献者位置信息时出错: {}", owner, repo, e);
    //         return Err(e.into());
    //     }
    // }

    // 创建GitHub API客户端
    let github_client = GitHubApiClient::new();

    // 获取仓库贡献者
    let contributors = github_client
        .get_all_repository_contributors(owner, repo)
        .await?;

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

fn is_shallow_repo(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(path)
        .output()
        .expect("Failed to run git");

    String::from_utf8_lossy(&output.stdout).trim() == "true"
}
