use git2::Repository;
use git2::{TreeWalkMode, TreeWalkResult};
use model::crate_info::{
    ApplicationVersion, DependsOn, HasDepVersion, HasVersion, LibraryVersion, UProgram, UVersion,
    Version,
};
use toml::Value;

use crate::utils::get_program_by_name;

#[allow(unused)]
#[derive(Debug, Default, Clone)]
pub(crate) struct Dependencies {
    pub(crate) crate_name: String,
    pub(crate) version: String,
    pub(crate) dependencies: Vec<(String, String)>,
}

/// a git repo contains different crates
#[allow(clippy::type_complexity)]
pub(crate) fn parse_all_versions_of_a_repo(
    repo: &Repository,
) -> (
    Vec<(HasVersion, UVersion, Version, HasDepVersion)>,
    Vec<DependsOn>,
) {
    let mut versions = vec![];
    let mut depends_on_vec: Vec<DependsOn> = vec![];

    let tags = repo.tag_names(None).expect("Could not retrieve tags");

    for tag_name in tags.iter().flatten() {
        let obj = repo
            .revparse_single(&("refs/tags/".to_owned() + tag_name))
            .expect("Couldn't find tag object");
        //println!("{:?}", obj);

        // convert annotated and light-weight tag into commit
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
        let all_packages_dependencies = parse_a_repo_of_a_version(repo, &tree);
        //debug!("{:?}", all_packages_dependencies);
        for dependencies in all_packages_dependencies {
            let name = dependencies.crate_name;
            let version = dependencies.version;
            let (program, uprogram) = match get_program_by_name(&name) {
                Some((program, uprogram)) => (program, uprogram),
                None => {
                    //FIXME: rename along with versions updates
                    continue;
                }
            };

            let has_version = HasVersion {
                SRC_ID: program.id.clone(),
                DST_ID: program.id.clone(), //FIXME: version id undecided
            };

            let dep_version = Version {
                name_and_version: name.clone() + &version,
            };

            #[allow(non_snake_case)]
            let SRC_ID = program.id.clone();
            #[allow(non_snake_case)]
            let DST_ID = name.clone() + &version;
            let has_dep_version = HasDepVersion { SRC_ID, DST_ID };

            let islib = matches!(uprogram, UProgram::Library(_));
            if islib {
                let version = LibraryVersion {
                    id: program.id.clone(),
                    name: name.clone(),
                    version: version.clone(),
                    documentation: "???".to_string(),
                };
                versions.push((
                    has_version,
                    UVersion::LibraryVersion(version),
                    dep_version,
                    has_dep_version,
                ));
            } else {
                let version = ApplicationVersion {
                    id: program.id.clone(),
                    name: name.clone(),
                    version: version.clone(),
                };
                versions.push((
                    has_version,
                    UVersion::ApplicationVersion(version),
                    dep_version,
                    has_dep_version,
                ));
            }

            for (dependency_name, dependency_version) in dependencies.dependencies {
                #[allow(non_snake_case)]
                let SRC_ID = name.clone() + &version;

                #[allow(non_snake_case)]
                let DST_ID = dependency_name + &dependency_version;
                let depends_on = DependsOn { SRC_ID, DST_ID };
                depends_on_vec.push(depends_on);
            }
        }
    }

    (versions, depends_on_vec)
}

/// for a given commit(version), walk all the package
fn parse_a_repo_of_a_version<'repo>(
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

            let dependencies = parse_a_package_of_a_version(content).expect("failed to parse toml");

            res.push(dependencies);

            return TreeWalkResult::Ok; // Found the file, stop walking
        }
        TreeWalkResult::Ok
    })
    .expect("Failed to walk the tree");

    res
}

fn parse_a_package_of_a_version(cargo_toml_content: &str) -> Option<Dependencies> {
    match cargo_toml_content.parse::<Value>() {
        Ok(toml) => {
            if let Some(package) = toml.get("package") {
                if let Some(crate_name) = package.get("name") {
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
                                } else if let Some(ver_tab) = val.as_table() {
                                    if let Some(val) = ver_tab.get("version") {
                                        if let Some(version) = val.as_str() {
                                            dependencies.push((name.clone(), version.to_owned()));
                                        }
                                    }
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
                    return Some(dependencies);
                }
            }
        }
        Err(_) => println!("Failed to parse Cargo.toml for {:?}", cargo_toml_content),
    }
    None
}
