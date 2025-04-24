use std::path::PathBuf;

use regex::Regex;
use sha2::{Digest, Sha256};

pub fn repo_dir(base_dir: PathBuf, owner: &str, repo: &str) -> PathBuf {
    let hash_hex = calculate_hash(owner);
    let d1 = &hash_hex[0..2];
    let d2 = &hash_hex[2..4];
    base_dir.join(PathBuf::from(format!("{}/{}/{}/{}", d1, d2, owner, repo)))
}

pub fn calculate_hash(owner: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(owner.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)
}

pub fn parse_to_owner_and_repo(repository: &str) -> Option<(String, String)> {
    let re = Regex::new(r"github\.com/([^/]+)/([^/]+)").unwrap();
    if let Some(captures) = re.captures(repository) {
        let owner = &captures[1];
        let repo = normalize_repo_url(&captures[2]);
        return Some((owner.to_owned(), repo.to_owned()));
    }
    None
}

pub fn normalize_repo_url(repo: &str) -> String {
    let re_git_suffix = Regex::new(r"(?i)\.git$").unwrap();
    let re_trailing_slash = Regex::new(r"/+$").unwrap();
    let re_fragment = Regex::new(r"#.*$").unwrap();

    let result = re_git_suffix.replace(repo, "");
    let result = re_trailing_slash.replace(&result, "");
    let result = re_fragment.replace(&result, "");

    result.to_string()
}

#[cfg(test)]
mod test {
    use crate::utils::parse_to_owner_and_repo;

    #[test]
    fn test_normalize_url() {
        let url1 = String::from("https://github.com/user/repo.git");
        let url2 = String::from("https://github.com/user/repo/");
        let url3 = String::from("https://github.com/user/repo#readme/a/b/c");
        assert_eq!(parse_to_owner_and_repo(&url1).unwrap().1, "repo");
        assert_eq!(parse_to_owner_and_repo(&url2).unwrap().1, "repo");
        assert_eq!(parse_to_owner_and_repo(&url3).unwrap().1, "repo");

        let fail_url1 = String::from("https://github.com/user/");
        assert!(parse_to_owner_and_repo(&fail_url1).is_none());
    }
}
