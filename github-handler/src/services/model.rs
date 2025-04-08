use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub _created_at: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQLResponse {
    pub data: Option<SearchData>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchData {
    pub search: SearchResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub edges: Vec<Edge>,
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub node: Repository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

// 解析提交数据
#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    pub login: String,
    pub id: i64,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
pub struct CommitInfo {
    pub _author: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommitDetail {
    pub author: Option<CommitInfo>,
}

#[derive(Debug, Deserialize)]
pub struct CommitData {
    pub author: Option<CommitAuthor>,
    pub commit: CommitDetail,
}
