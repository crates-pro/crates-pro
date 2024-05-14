use crate::utils::{get_program_by_name, name_join_version};
use crate::ImportDriver;
use git2::Repository;
use git2::{TreeWalkMode, TreeWalkResult};
use model::crate_info::{
    ApplicationVersion, DependsOn, HasDepVersion, HasVersion, LibraryVersion, UProgram, UVersion,
    Version,
};
use std::collections::HashMap;
use toml::Value;

#[allow(unused)]
#[derive(Debug, Default, Clone)]
pub(crate) struct Dependencies {
    pub(crate) crate_name: String,
    pub(crate) version: String,
    pub(crate) dependencies: Vec<(String, String)>,
}

impl ImportDriver {
    /// a git repo contains different crates
    #[allow(clippy::type_complexity)]
    pub(crate) fn parse_all_versions_of_a_repo(
        &mut self,
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
            let all_packages_dependencies = self.parse_a_repo_of_a_version(repo, &tree);
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
                    name_and_version: name_join_version(&name, &version),
                };

                #[allow(non_snake_case)]
                let SRC_ID = program.id.clone();
                #[allow(non_snake_case)]
                let DST_ID = name_join_version(&name, &version);
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
                    let SRC_ID = name_join_version(&name, &version);

                    #[allow(non_snake_case)]
                    let DST_ID = name_join_version(&dependency_name, &dependency_version);
                    let depends_on = DependsOn { SRC_ID, DST_ID };
                    depends_on_vec.push(depends_on);
                }
            }
        }

        (versions, depends_on_vec)
    }

    /// for a given commit(version), walk all the package
    fn parse_a_repo_of_a_version<'repo>(
        &mut self,
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

                let content = std::str::from_utf8(blob.content())
                    .expect("Cargo.toml content is not valid UTF-8");

                let dependencies = self
                    .parse_a_package_of_a_version(content)
                    .expect("failed to parse toml");

                res.push(dependencies);

                return TreeWalkResult::Ok; // Found the file, stop walking
            }
            TreeWalkResult::Ok
        })
        .expect("Failed to walk the tree");

        res
    }

    fn parse_a_package_of_a_version(&mut self, cargo_toml_content: &str) -> Option<Dependencies> {
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

                        self.version_parser.insert_version(&crate_name, &version);

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
                                                dependencies
                                                    .push((name.clone(), version.to_owned()));
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
}

#[derive(Default, Debug)]
pub(crate) struct VersionParser {
    version_map: HashMap<String, Vec<String>>,
}

impl VersionParser {
    pub fn insert_version(&mut self, crate_name: &str, version: &str) {
        self.version_map
            .entry(crate_name.to_string())
            .or_default()
            .push(version.to_string());
    }

    pub fn find_latest_matching_version(
        &self,
        target_lib: &str,
        target_version: &str,
    ) -> Option<String> {
        if let Some(lib_map) = self.version_map.get(target_lib) {
            // if the lib exists
            let req_str = if target_version.contains('.') {
                format!("^{}", target_version)
            } else {
                format!("{}.*", target_version)
            };

            let requirement = match semver::VersionReq::parse(&req_str) {
                Ok(req) => req,
                Err(_) => return None, // 如果无法解析为有效的版本请求，则返回 None
            };

            let mut matching_versions: Vec<semver::Version> = lib_map
                .iter()
                .filter_map(|ver| semver::Version::parse(ver).ok()) // 将所有版本字符串解析为 Version 对象
                .filter(|ver| requirement.matches(ver))
                .collect();

            // Sort the matched versions and return the last (largest) one
            matching_versions.sort();
            return matching_versions.last().map(|v| v.to_string());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::VersionParser;

    #[test]
    fn test_insert_and_find_version() {
        let mut parser = VersionParser::default();
        parser.insert_version("crate_a", "1.0.1");
        parser.insert_version("crate_a", "1.1.1");
        parser.insert_version("crate_a", "1.2.1");
        parser.insert_version("crate_a", "1.2.2");

        // Test finding the latest exact version
        assert_eq!(
            parser.find_latest_matching_version("crate_a", "1.2"),
            Some("1.2.2".to_string())
        );
        assert_eq!(
            parser.find_latest_matching_version("crate_a", "1"),
            Some("1.2.2".to_string())
        );

        // Test finding versions when there's no match
        assert_eq!(parser.find_latest_matching_version("crate_a", "2.0"), None);

        // Test finding versions with a precise match
        parser.insert_version("crate_b", "2.0.0");
        parser.insert_version("crate_b", "2.0.1");
        assert_eq!(
            parser.find_latest_matching_version("crate_b", "2.0.1"),
            Some("2.0.1".to_string())
        );

        assert_eq!(
            parser.find_latest_matching_version("crate_b", "2"),
            Some("2.0.1".to_string())
        );
        assert_eq!(parser.find_latest_matching_version("crate_c", "2"), None);
    }
}
