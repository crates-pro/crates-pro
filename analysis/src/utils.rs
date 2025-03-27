//use url::Url;

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
pub(crate) async fn extract_namespace(url_str: &str) -> Result<String, String> {
    tracing::info!("enter extract_namespace");
    /// auxiliary function
    fn remove_dot_git_suffix(input: &str) -> String {
        let input = if input.ends_with('/') {
            input.strip_suffix('/').unwrap()
        } else {
            input
        };

        let input = if input.ends_with(".git") {
            input.strip_suffix(".git").unwrap().to_string()
        } else {
            input.to_string()
        };
        input
    }

    let url = remove_dot_git_suffix(url_str);

    tracing::info!("finish get url:{:?}", url);
    // /tokio-rs/tokio

    let segments: Vec<&str> = url.split("/").collect();
    tracing::info!("finish get segments");

    // github URLs is of the format "/user/repo"
    if segments.len() < 2 {
        return Err(format!(
            "URL {} does not include a namespace and a repository name",
            url_str
        ));
    }

    // join owner name and repo name
    let namespace = format!(
        "{}/{}",
        segments[segments.len() - 2],
        segments[segments.len() - 1]
    );

    Ok(namespace)
}
