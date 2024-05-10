use git2::Repository;
use git2::{TreeWalkMode, TreeWalkResult};
use model::crate_info::UVersion;
use toml::Value;

#[allow(unused)]
#[derive(Debug, Default, Clone)]
pub(crate) struct Dependencies {
    pub(crate) crate_name: String,
    pub(crate) version: String,
    pub(crate) dependencies: Vec<(String, String)>,
}

/// a git repo contains different crates
pub(crate) fn extract_all_tags(repo: &Repository) -> Vec<UVersion> {
    let versions: Vec<UVersion> = vec![];
    let tags = repo.tag_names(None).expect("Could not retrieve tags");

    for tag_name in tags.iter().flatten() {
        let obj = repo
            .revparse_single(&("refs/tags/".to_owned() + tag_name))
            .expect("Couldn't find tag object");
        println!("{:?}", obj);

        // 尝试将对象作为注释标签处理
        let commit = if let Some(tag) = obj.as_tag() {
            tag.target()
                .expect("Couldn't get tag target")
                .peel_to_commit()
                .expect("Couldn't peel to commit")
        } else if let Some(commit) = obj.as_commit() {
            commit.clone()
        } else {
            panic!("Error!");
        };

        let tree = commit.tree().expect("Couldn't get the tree"); // for each version of the git repo

        // FIXME: deal with different formats
        // parse the version, walk all the packages
        let _all_packages_dependencies = find_and_parse_cargo_toml(repo, &tree);

        // let version = tag_name.to_string();
        // if is_lib {
        //     let version = LibraryVersion {
        //         id: id.clone(),
        //         name: name.clone(),
        //         version,
        //         documentation: "???".to_string(),
        //     };
        //     versions.push(UVersion::LibraryVersion(version));
        // } else {
        //     let version = ApplicationVersion {
        //         id: id.clone(),
        //         name: name.clone(),
        //         version,
        //     };
        //     versions.push(UVersion::ApplicationVersion(version));
        // }
    }

    versions
}

/// for a given commit(version), walk all the package
fn find_and_parse_cargo_toml<'repo>(
    repo: &'repo Repository,
    tree: &'repo git2::Tree,
) -> Vec<Dependencies> {
    let mut res = Vec::new();

    // Walk the tree to find Cargo.toml
    tree.walk(TreeWalkMode::PreOrder, |_, entry| {
        if entry.name() == Some("Cargo.toml") {
            // for each Cargo.toml in repo of given commit
            let obj = entry
                .to_object(repo)
                .expect("Failed to convert TreeEntry to Object");
            let blob = obj.as_blob().expect("Failed to interpret object as blob");
            let content =
                std::str::from_utf8(blob.content()).expect("Cargo.toml content is not valid UTF-8");

            //println!("xxx");
            match content.parse::<Value>() {
                Ok(toml) => {
                    //println!("yyy: {:#?}", toml);
                    if let Some(package) = toml.get("package") {
                        if let Some(crate_name) = package.get("name") {
                            //println!("zzz");
                            let crate_name = crate_name.as_str().unwrap().to_string();
                            let version = package
                                .get("version")
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_string();

                            let mut dependencies = vec![];

                            if let Some(dep_table) = toml.get("dependencies") {
                                if let Some(deps_table) = dep_table.as_table() {
                                    for (name, val) in deps_table {
                                        if let Some(version) = val.as_str() {
                                            //FIXME:
                                            dependencies.push((name.clone(), version.to_owned()));
                                        }
                                    }
                                }
                            }

                            let dependencies = Dependencies {
                                crate_name,
                                version,
                                dependencies,
                            };
                            println!("{:?}", dependencies);
                            res.push(dependencies)
                        }
                    }
                }
                Err(_) => println!("Failed to parse Cargo.toml for {:?}", entry.name()),
            }

            return TreeWalkResult::Ok; // Found the file, stop walking
        }
        TreeWalkResult::Ok
    })
    .expect("Failed to walk the tree");

    res
}
