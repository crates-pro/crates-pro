use std::sync::Arc;

use entity::{
    contributor_location, github_sync_status, github_user,
    programs::{self},
    repository_contributor,
};
use futures::Stream;
use model::github::{ContributorAnalysis, GitHubUser};
use sea_orm::{
    sea_query::OnConflict, ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait,
    DatabaseConnection, DbErr, EntityTrait, QueryFilter, Statement,
};
use tracing::{info, warn};

// 贡献者详情返回结果
#[derive(Debug, Clone)]
pub struct ContributorDetail {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub contributions: i32,
    pub location: Option<String>,
}

// 中国贡献者统计结果
#[derive(Debug, Clone)]
pub struct ChinaContributorStats {
    pub total_contributors: i64,
    pub china_contributors: i64,
    pub china_percentage: f64,
    pub china_contributors_details: Vec<ContributorDetail>,
}

#[derive(Clone)]
pub struct GithubHanlderStorage {
    pub connection: Arc<DatabaseConnection>,
}

impl GithubHanlderStorage {
    pub fn get_connection(&self) -> &DatabaseConnection {
        &self.connection
    }

    pub async fn new(connection: Arc<DatabaseConnection>) -> Self {
        GithubHanlderStorage { connection }
    }

    pub async fn save_programs(&self, models: Vec<programs::ActiveModel>) -> Result<(), DbErr> {
        let on_conflict = OnConflict::column(programs::Column::GithubUrl)
            .do_nothing()
            .to_owned();

        programs::Entity::insert_many(models)
            .on_conflict(on_conflict)
            .do_nothing()
            .exec(self.get_connection())
            .await
            .unwrap();
        Ok(())
    }

    pub async fn query_programs_stream(
        &self,
    ) -> Result<impl Stream<Item = Result<programs::Model, DbErr>> + Send + '_, DbErr> {
        programs::Entity::find().stream(self.get_connection()).await
    }

    pub async fn save_github_sync_status(
        &self,
        model: github_sync_status::ActiveModel,
    ) -> Result<github_sync_status::ActiveModel, DbErr> {
        model.save(self.get_connection()).await
    }

    pub async fn get_github_sync_status_by_date(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Option<github_sync_status::Model>, DbErr> {
        let result = github_sync_status::Entity::find()
            .filter(github_sync_status::Column::StartDate.eq(start_date))
            .filter(github_sync_status::Column::EndDate.eq(end_date))
            .one(self.get_connection())
            .await?;
        Ok(result)
    }

    // 存储GitHub用户
    pub async fn store_user(&self, user: &GitHubUser) -> Result<i32, DbErr> {
        info!("存储GitHub用户: {}", user.login);

        // 查询用户是否已存在
        let existing_user = github_user::Entity::find()
            .filter(github_user::Column::GithubId.eq(user.id))
            .one(self.get_connection())
            .await?;

        // 如果用户已存在，返回ID
        if let Some(existing) = existing_user {
            info!("用户 {} 已存在，ID: {}", user.login, existing.id);
            return Ok(existing.id);
        }

        // 用户不存在，创建新用户
        info!("创建新用户: {}", user.login);
        let user_model = github_user::ActiveModel::from(user.clone());
        let res = user_model.insert(self.get_connection()).await?;

        Ok(res.id)
    }

    // 根据用户名查找用户ID
    pub async fn get_user_id_by_name(&self, login: &str) -> Result<Option<i32>, DbErr> {
        info!("通过登录名查找用户ID: {}", login);

        let user = github_user::Entity::find()
            .filter(github_user::Column::Login.eq(login))
            .one(self.get_connection())
            .await?;

        Ok(user.map(|u| u.id))
    }

    // 根据仓库所有者和名称获取仓库ID
    pub async fn get_repository_id(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<String>, DbErr> {
        info!("获取仓库ID: {}/{}", owner, repo);

        // 直接查询github_url字段
        let programs = programs::Entity::find()
            .filter(
                programs::Column::GithubUrl
                    .contains(format!("{}/{}", owner, repo))
                    .or(programs::Column::GithubUrl.contains(format!("{}/{}.git", owner, repo))),
            )
            .all(self.get_connection())
            .await?;

        if !programs.is_empty() {
            info!("找到仓库 {}/{}, ID: {}", owner, repo, programs[0].id);
            return Ok(Some(programs[0].id.to_string()));
        }

        // 如果没有找到，尝试直接通过名称匹配
        let programs_by_name = programs::Entity::find()
            .filter(programs::Column::Name.eq(repo))
            .all(self.get_connection())
            .await?;

        if !programs_by_name.is_empty() {
            info!("通过名称找到仓库 {}, ID: {}", repo, programs_by_name[0].id);
            return Ok(Some(programs_by_name[0].id.to_string()));
        }

        warn!("未找到仓库 {}/{}", owner, repo);
        Ok(None)
    }

    // 存储仓库贡献者
    pub async fn store_contributor(
        &self,
        repository_id: &str,
        user_id: i32,
        contributions: i32,
    ) -> Result<(), DbErr> {
        info!(
            "存储贡献者关系: 仓库ID={}, 用户ID={}, 提交数={}",
            repository_id, user_id, contributions
        );

        // 检查是否存在现有记录
        let existing = repository_contributor::Entity::find()
            .filter(repository_contributor::Column::RepositoryId.eq(repository_id))
            .filter(repository_contributor::Column::UserId.eq(user_id))
            .one(self.get_connection())
            .await?;

        if let Some(existing) = existing {
            // 已存在，更新贡献数
            if existing.contributions != contributions {
                let mut model: repository_contributor::ActiveModel = existing.clone().into();
                model.contributions = Set(contributions);
                model.updated_at = Set(chrono::Utc::now().naive_utc());
                model.update(self.get_connection()).await?;
                info!(
                    "更新贡献者贡献数: {} -> {}",
                    existing.contributions, contributions
                );
            } else {
                info!("贡献者记录已存在且贡献数相同, 跳过更新");
            }
        } else {
            // 不存在，创建新记录
            let now = chrono::Utc::now().naive_utc();
            let contributor = repository_contributor::ActiveModel {
                id: Default::default(),
                repository_id: Set(repository_id.to_string()),
                user_id: Set(user_id),
                contributions: Set(contributions),
                inserted_at: Set(now),
                updated_at: Set(now),
            };

            contributor.insert(self.get_connection()).await?;
            info!("创建新的贡献者记录");
        }

        Ok(())
    }

    // 查询仓库的顶级贡献者
    pub async fn query_top_contributors(
        &self,
        repository_id: &str,
    ) -> Result<Vec<ContributorDetail>, DbErr> {
        info!("查询仓库 ID={} 的顶级贡献者", repository_id);

        // 构建查询
        let query = "
                SELECT gu.github_id, gu.login, gu.name, rc.contributions, gu.location
                FROM repository_contributor rc
                JOIN github_user gu ON rc.user_id = gu.id
                WHERE rc.repository_id = $1
                ORDER BY rc.contributions DESC
                LIMIT 20
            ";

        // 执行查询
        let result = self
            .get_connection()
            .query_all(Statement::from_sql_and_values(
                self.get_connection().get_database_backend(),
                query,
                [repository_id.into()],
            ))
            .await?;

        // 解析结果
        let mut contributors = Vec::new();
        for row in result {
            let id: i64 = row.try_get("", "github_id")?;
            let login: String = row.try_get("", "login")?;
            let name: Option<String> = row.try_get("", "name")?;
            let contributions: i32 = row.try_get("", "contributions")?;
            let location: Option<String> = row.try_get("", "location")?;

            contributors.push(ContributorDetail {
                id,
                login,
                name,
                contributions,
                location,
            });
        }

        info!("找到 {} 个顶级贡献者", contributors.len());
        Ok(contributors)
    }

    // 存储贡献者位置信息
    pub async fn store_contributor_location(
        &self,
        repository_id: &str,
        user_id: i32,
        analysis: &ContributorAnalysis,
    ) -> Result<(), DbErr> {
        info!(
            "存储贡献者位置信息: 仓库ID={}, 用户ID={}",
            repository_id, user_id
        );

        // 通过conversion trait转换
        let mut cl = contributor_location::ActiveModel::from(analysis);
        cl.user_id = Set(user_id);
        cl.repository_id = Set(repository_id.to_owned());
        cl.insert(self.get_connection()).await?;

        info!("贡献者位置信息已存储");
        Ok(())
    }

    // 获取仓库的中国贡献者统计
    pub async fn get_repository_china_contributor_stats(
        &self,
        repository_id: &str,
    ) -> Result<ChinaContributorStats, DbErr> {
        info!("获取仓库 ID={} 的中国贡献者统计", repository_id);

        // 查询中国贡献者统计
        let stats_query = "
                SELECT 
                    COUNT(*) as total_contributors,
                    COALESCE(SUM(CASE WHEN is_from_china THEN 1 ELSE 0 END), 0) as china_contributors
                FROM contributor_location
                WHERE repository_id = $1
            ";

        let maybe_result = self
            .get_connection()
            .query_one(Statement::from_sql_and_values(
                self.get_connection().get_database_backend(),
                stats_query,
                [repository_id.into()],
            ))
            .await?;

        // 如果没有结果，返回空值
        let stats_result = match maybe_result {
            Some(result) => result,
            None => {
                return Ok(ChinaContributorStats {
                    total_contributors: 0,
                    china_contributors: 0,
                    china_percentage: 0.0,
                    china_contributors_details: Vec::new(),
                });
            }
        };

        let total_contributors: i64 = stats_result.try_get("", "total_contributors")?;
        let china_contributors: i64 = stats_result.try_get("", "china_contributors")?;

        let china_percentage = if total_contributors > 0 {
            (china_contributors as f64 / total_contributors as f64) * 100.0
        } else {
            0.0
        };

        // 查询中国贡献者详情
        let china_details_query = "
                SELECT gu.github_id, gu.login, gu.name, rc.contributions, gu.location
                FROM contributor_location cl
                JOIN github_user gu ON cl.user_id = gu.id
                JOIN repository_contributor rc ON cl.user_id = rc.user_id AND cl.repository_id = rc.repository_id
                WHERE cl.repository_id = $1 AND cl.is_from_china = true
                ORDER BY rc.contributions DESC
                LIMIT 10
            ";

        let china_details = self
            .get_connection()
            .query_all(Statement::from_sql_and_values(
                self.get_connection().get_database_backend(),
                china_details_query,
                [repository_id.into()],
            ))
            .await?;

        let mut china_contributors_details = Vec::new();
        for row in china_details {
            let id: i64 = row.try_get("", "github_id")?;
            let login: String = row.try_get("", "login")?;
            let name: Option<String> = row.try_get("", "name")?;
            let contributions: i32 = row.try_get("", "contributions")?;
            let location: Option<String> = row.try_get("", "location")?;

            china_contributors_details.push(ContributorDetail {
                id,
                login,
                name,
                contributions,
                location,
            });
        }

        Ok(ChinaContributorStats {
            total_contributors,
            china_contributors,
            china_percentage,
            china_contributors_details,
        })
    }

    // 根据仓库ID检查是否存在贡献者位置信息
    pub async fn has_contributor_location(&self, repository_id: &str) -> Result<bool, DbErr> {
        info!("检查仓库 ID={} 是否存在贡献者位置信息", repository_id);

        // 查询仓库是否存在贡献者位置信息
        let query = "
            SELECT EXISTS(
                SELECT 1 
                FROM contributor_location 
                WHERE repository_id = $1
                LIMIT 1
            ) as exists_flag
        ";

        let result = self
            .get_connection()
            .query_one(Statement::from_sql_and_values(
                self.get_connection().get_database_backend(),
                query,
                [repository_id.into()],
            ))
            .await?;

        match result {
            Some(row) => {
                let exists: bool = row.try_get("", "exists_flag")?;
                info!(
                    "仓库 ID={} 的贡献者位置信息{}存在",
                    repository_id,
                    if exists { "" } else { "不" }
                );
                Ok(exists)
            }
            None => {
                warn!("查询仓库 ID={} 的贡献者位置信息时出错", repository_id);
                Ok(false)
            }
        }
    }
}
