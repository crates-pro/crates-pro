use crate::git::get_all_git_tags;
use crate::utils::{get_program_by_name, name_join_version};
use crate::ImportDriver;
use git2::Repository;
use git2::{TreeWalkMode, TreeWalkResult};
use model::tugraph_model::{
    ApplicationVersion, CrateType2Idx, DependsOn, HasDepVersion, HasVersion, LibraryVersion,
    UVersion, Version,
};
use std::collections::HashMap;
use toml::Value;

/// A representation for the info
/// extracted from a `cargo.toml` file
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
    pub(crate) async fn parse_all_versions_of_a_repo(
        &mut self,
        repo: &Repository,
    ) -> (
        Vec<(HasVersion, UVersion, Version, HasDepVersion)>,
        Vec<DependsOn>,
    ) {
        let mut versions = vec![];
        let mut depends_on_vec: Vec<DependsOn> = vec![];

        let trees = get_all_git_tags(repo).await;

        for tree in trees.iter() {
            // FIXME: deal with different formats
            // parse the version, walk all the packages
            let all_packages_dependencies = self.parse_a_repo_of_a_version(repo, tree).await;
            for dependencies in all_packages_dependencies {
                let name = dependencies.crate_name.clone();
                let version = dependencies.version.clone();
                let (program, uprogram) = match get_program_by_name(&name) {
                    Some((program, uprogram)) => (program, uprogram),
                    None => {
                        // continue, dont parse
                        continue;
                    }
                };

                self.version_updater.update_depends_on(&dependencies).await;

                let has_version = HasVersion {
                    SRC_ID: program.id.clone(),
                    DST_ID: name_join_version(&name, &version), //FIXME: version id undecided
                };

                let dep_version = Version {
                    name_and_version: name_join_version(&name, &version),
                };

                #[allow(non_snake_case)]
                let SRC_ID = name_join_version(&name, &version);
                #[allow(non_snake_case)]
                let DST_ID = name_join_version(&name, &version);
                let has_dep_version = HasDepVersion { SRC_ID, DST_ID };

                let islib = uprogram.index() == 0;
                if islib {
                    let version = LibraryVersion::new(
                        program.id.clone(),
                        &name.clone(),
                        &version.clone(),
                        "???",
                    );
                    versions.push((
                        has_version,
                        UVersion::LibraryVersion(version),
                        dep_version,
                        has_dep_version,
                    ));
                } else {
                    let version =
                        ApplicationVersion::new(program.id.clone(), name.clone(), version.clone());
                    versions.push((
                        has_version,
                        UVersion::ApplicationVersion(version),
                        dep_version,
                        has_dep_version,
                    ));
                }

                depends_on_vec = self.version_updater.to_depends_on_edges().await;
            }
        }

        (versions, depends_on_vec)
    }

    /// for a given commit(version), walk all the package
    async fn parse_a_repo_of_a_version<'repo>(
        &self,
        repo: &'repo Repository,
        tree: &'repo git2::Tree<'repo>,
    ) -> Vec<Dependencies> {
        let mut res = Vec::new();

        //println!("{}", tree.len());
        // Walk the tree to find Cargo.toml
        tree.walk(TreeWalkMode::PostOrder, |_, entry| {
            //println!("{:?}", entry.name());
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
                    .unwrap_or_default();

                res.push(dependencies);
            }
            TreeWalkResult::Ok
        })
        .unwrap();

        res
    }

    fn parse_a_package_of_a_version(&self, cargo_toml_content: &str) -> Option<Dependencies> {
        match cargo_toml_content.parse::<Value>() {
            Ok(toml) => {
                if let Some(package) = toml.get("package") {
                    if let Some(crate_name) = package.get("name") {
                        let crate_name = crate_name.as_str()?.to_string();
                        let version = package.get("version")?.as_str()?.to_string();

                        // dedup
                        if self
                            .version_updater
                            .version_parser
                            .exists(&crate_name, &version)
                        {
                            return None;
                        }

                        let mut dependencies = vec![];

                        if let Some(dep_table) = toml.get("dependencies") {
                            if let Some(deps_table) = dep_table.as_table() {
                                for (name, val) in deps_table {
                                    if let Some(version) = val.as_str() {
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

                        if crate_name.as_str() == "ansi_term" {
                            println!("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
                        }
                        let dependencies = Dependencies {
                            crate_name,
                            version,
                            dependencies,
                        };

                        return Some(dependencies);
                    }
                }
            }
            Err(_) => error!("Failed to parse Cargo.toml for {:?}", cargo_toml_content),
        }
        None
    }
}

#[derive(Debug, Default)]
pub struct VersionUpdater {
    /// a reverse record: who depends on the key?
    pub reverse_depends_on_map: HashMap<String, Vec<(String, model::general_model::Version)>>,

    /// a actual map: a crate **actually** depends on which?
    /// it is used to build `depends_on` edges.
    pub actually_depends_on_map:
        HashMap<model::general_model::Version, Vec<model::general_model::Version>>,

    pub version_parser: VersionParser,
}

impl VersionUpdater {
    pub async fn to_depends_on_edges(&self) -> Vec<DependsOn> {
        let mut edges = vec![];
        for (src, dsts) in &self.actually_depends_on_map {
            for dst in dsts {
                #[allow(non_snake_case)]
                let SRC_ID = name_join_version(&src.name, &src.version);

                #[allow(non_snake_case)]
                let DST_ID = name_join_version(&dst.name, &dst.version);
                let depends_on = DependsOn { SRC_ID, DST_ID };
                edges.push(depends_on);
            }
        }
        edges
    }

    /// Given a dependency list,
    pub async fn update_depends_on(&mut self, info: &Dependencies) {
        self.version_parser
            .insert_version(&info.crate_name, &info.version)
            .await;
        let cur_release = model::general_model::Version::new(&info.crate_name, &info.version);
        self.ensure_dependencies(&cur_release, info).await;
        self.ensure_dependents(&cur_release).await;
    }

    async fn ensure_dependencies(
        &mut self,
        cur_release: &model::general_model::Version,
        info: &Dependencies,
    ) {
        for (name, version) in &info.dependencies {
            //let dep = model::general_model::Version::new(&name, &version);
            self.insert_reverse_dep(name, version, &cur_release.name, &cur_release.version)
                .await;
        }

        // a new version should not exist before.
        assert!(!self.actually_depends_on_map.contains_key(cur_release));
        let cur_dependencies = self.search_dependencies(info).await;
        self.actually_depends_on_map
            .insert(cur_release.clone(), cur_dependencies);
    }

    async fn search_dependencies(&self, info: &Dependencies) -> Vec<model::general_model::Version> {
        let mut res: Vec<model::general_model::Version> = vec![];
        for (dependency_name, dependency_version) in &info.dependencies {
            let version_option = self
                .version_parser
                .find_latest_matching_version(dependency_name, dependency_version)
                .await;

            if let Some(dependency_actual_version) = &version_option {
                let dependency =
                    model::general_model::Version::new(dependency_name, dependency_actual_version);
                res.push(dependency);
            }
        }
        res
    }

    pub async fn ensure_dependents(&mut self, cur_release: &model::general_model::Version) {
        let sem_ver = semver::Version::parse(&cur_release.version)
            .unwrap_or_else(|_| panic!("failed to parse version {}", &cur_release.version));
        let wrapped_reverse_map = self.reverse_depends_on_map.get(&cur_release.name);
        if let Some(reverse_map) = wrapped_reverse_map {
            for (required_version, reverse_dep) in reverse_map {
                let requirement = match semver::VersionReq::parse(required_version) {
                    Ok(req) => req,
                    Err(_) => {
                        tracing::error!("failed to transform to VersionReq");
                        continue; // 如果无法解析为有效的版本请求，则返回 None
                    }
                };

                if requirement.matches(&sem_ver) {
                    if let Some(v) = self.actually_depends_on_map.get_mut(reverse_dep) {
                        let mut found = false;
                        for x in &mut *v {
                            if x.name == cur_release.name {
                                found = true;
                                let prev_sem_ver = semver::Version::parse(&x.version).unwrap();
                                if sem_ver < prev_sem_ver {
                                    //replace
                                    x.version.clone_from(&cur_release.version);
                                }
                                //found break;
                                break;
                            }
                        }
                        if !found {
                            v.push(model::general_model::Version::new(
                                &cur_release.name,
                                &cur_release.version,
                            ));
                        }
                    } else {
                        // No vec
                        self.actually_depends_on_map.insert(
                            reverse_dep.clone(),
                            vec![model::general_model::Version::new(
                                &cur_release.name,
                                &cur_release.version,
                            )],
                        );
                    }
                }
            }
        }
    }

    /// insert (dependency, dependent)
    /// notice that: dependent is unique, but dependency should be newest.
    pub async fn insert_reverse_dep(
        &mut self,
        dependency_name: &str,
        dependency_version: &str,
        dependent_name: &str,
        dependent_version: &str,
    ) {
        //let dependency = model::general_model::Version::new(dependency_name, dependency_version);
        let dependent = model::general_model::Version::new(dependent_name, dependent_version);
        self.reverse_depends_on_map
            .entry(dependency_name.to_string())
            .or_default()
            .push((dependency_version.to_string(), dependent));
    }
}

#[derive(Default, Debug)]
pub(crate) struct VersionParser {
    version_map: HashMap<String, Vec<String>>,
}

impl VersionParser {
    pub async fn insert_version(&mut self, crate_name: &str, version: &str) {
        self.version_map
            .entry(crate_name.to_string())
            .or_default()
            .push(version.to_string());
    }

    pub(crate) fn exists(&self, name: &str, version: &str) -> bool {
        if let Some(map) = self.version_map.get(name) {
            return map.contains(&version.to_string());
        }
        false
    }

    pub(crate) async fn _remove(&mut self, name: &str) {
        self.version_map.remove(name);
    }

    pub async fn find_latest_matching_version(
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

    #[tokio::test]
    async fn test_insert_and_find_version() {
        let mut parser = VersionParser::default();
        parser.insert_version("crate_a", "1.0.1").await;
        parser.insert_version("crate_a", "1.1.1").await;
        parser.insert_version("crate_a", "1.2.1").await;
        parser.insert_version("crate_a", "1.2.2").await;

        // Test finding the latest exact version
        assert_eq!(
            parser.find_latest_matching_version("crate_a", "1.2").await,
            Some("1.2.2".to_string())
        );
        assert_eq!(
            parser.find_latest_matching_version("crate_a", "1").await,
            Some("1.2.2".to_string())
        );

        // Test finding versions when there's no match
        assert_eq!(
            parser.find_latest_matching_version("crate_a", "2.0").await,
            None
        );

        // Test finding versions with a precise match
        parser.insert_version("crate_b", "2.0.0").await;
        parser.insert_version("crate_b", "2.0.1").await;
        assert_eq!(
            parser
                .find_latest_matching_version("crate_b", "2.0.1")
                .await,
            Some("2.0.1".to_string())
        );

        assert_eq!(
            parser.find_latest_matching_version("crate_b", "2").await,
            Some("2.0.1".to_string())
        );
        assert_eq!(
            parser.find_latest_matching_version("crate_c", "2").await,
            None
        );
    }
}
