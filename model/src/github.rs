use chrono::{DateTime, Utc};
use entity::{contributor_location, github_user};
use sea_orm::ActiveValue::{NotSet, Set};
use serde::{Deserialize, Serialize};

// GitHub用户信息结构
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    pub public_repos: Option<i32>,
    pub followers: Option<i32>,
    pub following: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 转换函数，用于将GitHub API返回的用户转换为数据库模型
impl From<GitHubUser> for github_user::ActiveModel {
    fn from(user: GitHubUser) -> Self {
        let now = chrono::Utc::now().naive_utc();

        Self {
            id: NotSet,
            github_id: Set(user.id),
            login: Set(user.login),
            name: Set(user.name),
            email: Set(user.email),
            avatar_url: Set(user.avatar_url),
            company: Set(user.company),
            location: Set(user.location),
            bio: Set(user.bio),
            public_repos: Set(user.public_repos),
            followers: Set(user.followers),
            following: Set(user.following),
            created_at: Set(user.created_at.naive_utc()),
            updated_at: Set(user.updated_at.naive_utc()),
            inserted_at: Set(now),
            updated_at_local: Set(now),
        }
    }
}

// 贡献者信息结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contributor {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub contributions: i32,
    pub email: Option<String>,
}

// 贡献者分析结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContributorAnalysis {
    pub email: Option<String>,
    pub from_china: bool,
    pub common_timezone: String,
}

// 转换函数，将分析结果转换为数据库模型
impl From<&ContributorAnalysis> for contributor_location::ActiveModel {
    fn from(analysis: &ContributorAnalysis) -> Self {
        let now = chrono::Utc::now().naive_utc();

        Self {
            id: NotSet,
            is_from_china: Set(analysis.from_china),
            common_timezone: Set(Some(analysis.common_timezone.clone())),
            analyzed_at: Set(now),
            ..Default::default()
        }
    }
}
