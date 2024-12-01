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

#[derive(Debug)]
pub struct RecommendCrate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub downloads: i64,
}

impl<'a> SearchModule<'a> {
    pub async fn new(pg_client: &'a PgClient) -> Self {
        let table_name = env::var("TABLE_NAME").unwrap_or_else(|_| "crates".to_string());
        SearchModule {
            pg_client: pg_client,
            table_name,
        }
    }

    pub async fn search_crate(
        &self,
        keyword: &str,
        sort_by: SearchSortCriteria,
    ) -> Result<Vec<RecommendCrate>, Box<dyn std::error::Error>> {
        search_crate_without_ai(self.pg_client, &self.table_name, keyword, sort_by).await
    }
}

fn gen_search_sql(table_name: &str, sort_by: SearchSortCriteria) -> String {
    match sort_by {
        //TODO: 实现综合排序
        SearchSortCriteria::Comprehensive => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank,{0}.downloads
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY rank DESC",
                table_name
            )
        }
        SearchSortCriteria::Relavance => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank,{0}.downloads
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY rank DESC",
                table_name
            )
        }
        SearchSortCriteria::Downloads => {
            format!(
                "SELECT {0}.id, {0}.name, {0}.description, ts_rank({0}.tsv, to_tsquery($1)) AS rank, {0}.downloads
                FROM {0}
                WHERE {0}.tsv @@ to_tsquery($1)
                ORDER BY downloads DESC",
                table_name
            )
        }
    }
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
        let id: String = row.get("id");
        let name: String = row.get("name");
        let description: String = row.get("description");
        let downloads: i64 = row.get("downloads");
        recommend_crates.push(RecommendCrate {
            id,
            name,
            description,
            downloads,
        });
    }
    if let Some(pos) = recommend_crates
        .iter()
        .position(|recommend_crate| recommend_crate.name == keyword)
    {
        let direct_crate = recommend_crates.remove(pos);
        recommend_crates.insert(0, direct_crate);
    }


    Ok(recommend_crates)
}
