use semver::Version;
use std::env;
use tokio_postgres::Client as PgClient;

pub struct SearchModule<'a> {
    pg_client: &'a PgClient,
    table_name: String,
}

pub enum SearchSortCriteria {
    Comprehensive,
    Relavance,
    Downloads,
}

#[derive(Debug, Clone)]
pub struct RecommendCrate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub downloads: i64,
    pub namespace: String,
    pub max_version: String,
    pub rank: f32,
}

impl<'a> SearchModule<'a> {
    pub async fn new(pg_client: &'a PgClient) -> Self {
        let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "crates".to_string());
        SearchModule {
            pg_client,
            table_name,
        }
    }

    pub async fn search_crate(
        &self,
        keyword: &str,
        sort_by: SearchSortCriteria,
    ) -> Result<Vec<RecommendCrate>, Box<dyn std::error::Error>> {
        let mut crates =
            search_crate_without_ai(self.pg_client, &self.table_name, keyword, sort_by).await?;
        sort_crates(&mut crates);
        rearrange_crates(&mut crates, keyword);
        Ok(crates)
    }
}

fn gen_search_sql(table_name: &str, sort_by: SearchSortCriteria) -> String {
    match sort_by {
        //TODO: 实现综合排序
        SearchSortCriteria::Comprehensive => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank,{0}.downloads,{0}.namespace,{0}.max_version
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY rank DESC",
                table_name
            )
        }
        SearchSortCriteria::Relavance => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank,{0}.downloads,{0}.namespace,{0}.max_version
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY rank DESC",
                table_name
            )
        }
        SearchSortCriteria::Downloads => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank, {0}.downloads,{0}.namespace,{0}.max_version
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY downloads DESC",
                table_name
            )
        }
    }
}

fn sort_crates(crate_vec: &mut [RecommendCrate]) {
    let version_cmp = |a: &RecommendCrate, b: &RecommendCrate| {
        let version_a = Version::parse(&a.max_version);
        let version_b = Version::parse(&b.max_version);

        match (version_a, version_b) {
            (Ok(ver_a), Ok(ver_b)) => ver_b.cmp(&ver_a),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        }
    };

    crate_vec.sort_by(|a, b| {
        b.rank
            .partial_cmp(&a.rank)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| version_cmp(a, b))
    });
}

fn rearrange_crates(crates: &mut Vec<RecommendCrate>, keyword: &str) {
    let mut matching_crates: Vec<RecommendCrate> = Vec::new();
    crates.retain(|c| {
        if c.name == keyword {
            matching_crates.push(c.clone());
            false
        } else {
            true
        }
    });
    sort_crates(&mut matching_crates);
    crates.splice(0..0, matching_crates);
}

async fn search_crate_without_ai(
    client: &PgClient,
    table_name: &str,
    keyword: &str,
    sort_by: SearchSortCriteria,
) -> Result<Vec<RecommendCrate>, Box<dyn std::error::Error>> {
    let tsquery_keyword = keyword.replace(" ", " & ");
    let query = format!("{}:*", tsquery_keyword);

    let statement = gen_search_sql(table_name, sort_by);
    let rows = client.query(statement.as_str(), &[&query]).await?;
    let mut recommend_crates = Vec::<RecommendCrate>::new();

    for row in rows.iter() {
        let id: Option<String> = row.get("id");
        let name: Option<String> = row.get("name");
        let description: Option<String> = row.get("description");
        let downloads: Option<i64> = row.get("downloads");
        let namespace: Option<String> = row.get("namespace");
        let max_version: Option<String> = row.get("max_version");
        let rank: Option<f32> = row.get("rank");

        recommend_crates.push(RecommendCrate {
            id: id.unwrap_or_default(),
            name: name.unwrap_or_default(),
            description: description.unwrap_or_default(),
            downloads: downloads.unwrap_or(0),
            namespace: namespace.unwrap_or_default(),
            max_version: max_version.unwrap_or_default(),
            rank: rank.unwrap_or(0.0),
        });
    }

    Ok(recommend_crates)
}
