
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GitHubSearchResponse {
    pub items: Vec<Repository>,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub name: String,
    pub clone_url: String,
}


