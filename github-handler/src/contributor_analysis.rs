use chrono::{DateTime, FixedOffset};
use model::github::ContributorAnalysis;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Command as TokioCommand;
use tracing::{debug, error, info, warn};

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

/// 获取作者的所有提交
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

/// 分析仓库的所有贡献者
pub async fn analyze_repository_contributors(repo_path: &str) -> Vec<ContributorAnalysis> {
    let mut results = Vec::new();

    // 获取所有贡献者的邮箱
    let emails = match get_all_contributor_emails(repo_path).await {
        Some(emails) => emails,
        None => {
            error!("无法获取仓库贡献者邮箱: {}", repo_path);
            return results;
        }
    };

    info!("发现 {} 个贡献者邮箱", emails.len());

    // 分析每个贡献者
    for email in emails {
        if let Some(analysis) = analyze_contributor_timezone(repo_path, &email).await {
            debug!(
                "分析完成: {} (可能来自中国: {})",
                email,
                if analysis.from_china { "是" } else { "否" }
            );
            results.push(analysis);
        }
    }

    results
}

/// 获取所有贡献者的邮箱
async fn get_all_contributor_emails(repo_path: &str) -> Option<Vec<String>> {
    let output = TokioCommand::new("git")
        .current_dir(repo_path)
        .args(["shortlog", "-sen", "HEAD"])
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

    let mut emails = Vec::new();

    for line in lines {
        // 格式通常是: 123  Name <email@example.com>
        if let Some(email_start) = line.find('<') {
            if let Some(email_end) = line.find('>') {
                let email = line[email_start + 1..email_end].trim().to_string();
                emails.push(email);
            }
        }
    }

    Some(emails)
}

/// 生成仓库贡献者分析报告
pub async fn generate_contributors_report(repo_path: &str) -> ContributorsReport {
    info!("正在为仓库 {} 生成贡献者分析报告", repo_path);
    let all_analyses = analyze_repository_contributors(repo_path).await;

    // 获取中国贡献者和非中国贡献者的提交总数
    let china_commits: usize = all_analyses.iter().filter(|c| c.from_china).count();
    let non_china_commits: usize = all_analyses.len() - china_commits;
    let total_commits = china_commits + non_china_commits;

    let china_percentage = if total_commits > 0 {
        china_commits as f64 / total_commits as f64 * 100.0
    } else {
        0.0
    };

    ContributorsReport {
        total_contributors: all_analyses.len(),
        china_contributors_count: china_commits,
        non_china_contributors_count: non_china_commits,
        china_percentage,
        contributors: all_analyses,
    }
}

/// Error type for contributor analysis
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ContributorsReport {
    pub total_contributors: usize,
    pub china_contributors_count: usize,
    pub non_china_contributors_count: usize,
    pub china_percentage: f64,
    pub contributors: Vec<ContributorAnalysis>,
}

impl ContributorsReport {
    pub fn print_summary(&self) {
        info!("贡献者分析报告摘要:");
        info!("--------------------------------------------------");
        info!("总贡献者: {} 人", self.total_contributors);
        info!(
            "中国贡献者: {} 人 ({:.1}%)",
            self.china_contributors_count, self.china_percentage
        );
        info!(
            "非中国贡献者: {} 人 ({:.1}%)",
            self.non_china_contributors_count,
            100.0 - self.china_percentage
        );
        info!("--------------------------------------------------");
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
