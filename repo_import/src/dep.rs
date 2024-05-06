use git2::Repository;
use git2::{TreeWalkMode, TreeWalkResult};
use model::crate_info::{ApplicationVersion, LibraryVersion, UVersion};
use toml::Value;
pub(crate) fn extract_all_tags(
    repo: &Repository,
    id: String,
    name: String,
    is_lib: bool,
) -> Vec<UVersion> {
    let mut versions: Vec<UVersion> = vec![];
    let tags = repo.tag_names(None).expect("Could not retrieve tags");

    for tag_name in tags.iter() {
        if let Some(tag_name) = tag_name {
            let obj = repo
                .revparse_single(&("refs/tags/".to_owned() + tag_name))
                .expect("Couldn't find tag object");
            let tag = obj.as_tag().expect("Couldn't convert to tag");
            let commit = tag
                .target()
                .expect("Couldn't get tag target")
                .peel_to_commit()
                .expect("Couldn't peel to commit");
            let tree = commit.tree().expect("Couldn't get the tree");

            println!("Tag: {}", tag_name);
            match find_and_parse_cargo_toml(repo, &tree) {
                Some(deps) => {
                    println!("Dependencies:");
                    for (name, version) in deps {
                        println!("{}: {}", name, version);
                    }
                }
                None => println!("Could not find Cargo.toml"),
            }
        }

        let version = tag_name.unwrap_or("-").to_string();

        if is_lib {
            let version = LibraryVersion {
                id: id.clone(),
                name: name.clone(),
                version,
                documentation: "???".to_string(), // FIXME:
            };
            versions.push(UVersion::LibraryVersion(version));
        } else {
            let version = ApplicationVersion {
                id: id.clone(),
                name: name.clone(),
                version,
            };
            versions.push(UVersion::ApplicationVersion(version));
        }
    }

    versions
}
fn find_and_parse_cargo_toml<'repo>(
    repo: &'repo Repository,
    tree: &'repo git2::Tree,
) -> Option<Vec<(String, String)>> {
    let mut deps = Vec::new();

    // Walk the tree to find Cargo.toml
    tree.walk(TreeWalkMode::PreOrder, |_, entry| {
        if entry.name() == Some("Cargo.toml") {
            let obj = entry
                .to_object(repo)
                .expect("Failed to convert TreeEntry to Object");
            let blob = obj.as_blob().expect("Failed to interpret object as blob");
            let content =
                std::str::from_utf8(blob.content()).expect("Cargo.toml content is not valid UTF-8");

            match content.parse::<Value>() {
                Ok(toml) => {
                    if let Some(dep_table) = toml.get("dependencies") {
                        if let Some(deps_table) = dep_table.as_table() {
                            for (name, val) in deps_table {
                                if let Some(version) = val.as_str() {
                                    deps.push((name.clone(), version.to_owned()));
                                }
                            }
                        }
                    }
                }
                Err(_) => println!("Failed to parse Cargo.toml for {:?}", entry.name()),
            }

            return TreeWalkResult::Abort; // Found the file, stop walking
        }
        TreeWalkResult::Ok
    })
    .expect("Failed to walk the tree");

    if deps.is_empty() {
        None
    } else {
        Some(deps)
    }
}
