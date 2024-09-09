use url::Url;

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
pub(crate) async fn extract_namespace(url_str: &str) -> Result<String, String> {
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

    let url = Url::parse(&remove_dot_git_suffix(url_str))
        .map_err(|e| format!("Failed to parse URL {}: {}", url_str, e))?;

    // /tokio-rs/tokio
    let path_segments = url
        .path_segments()
        .ok_or("Cannot extract path segments from URL")?;

    let segments: Vec<&str> = path_segments.collect();
    //println!("{:?}", segments);

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
