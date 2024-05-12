use cargo_metadata::MetadataCommand;
use csv::Writer;
use model::crate_info::{Application, HasType, Library, Program, UProgram};
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fmt::Debug;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use toml::Value;
use url::Url;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::utils::{get_namespace_by_repo_path, insert_program_by_name};

// Given a project path, parse the metadata
pub(crate) fn extract_info_local(local_repo_path: PathBuf) -> Vec<(Program, HasType, UProgram)> {
    trace!("Parse repo {:?}", local_repo_path);
    let mut res = vec![];

    let id = Uuid::new_v4().to_string();

    // It is possible that there is no Cargo.toml file in the project root directory,
    // so the root directories are one level down
    // let (min_depth, max_depth) = if exists_cargo_toml(&local_repo_path) {
    //     (1, 2)
    // } else {
    //     (2, 3)
    // };

    // walk the directories of the project
    for entry in WalkDir::new(local_repo_path.clone())
        //.min_depth(min_depth) // owner/proj/Cargo.toml
        //.max_depth(max_depth) // workspace: owner/proj/Cargo.toml
        .into_iter()
        .filter_map(|x| x.ok())
    {
        let entry_path = entry.path();

        // if entry is Cargo.toml, ...
        if entry_path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
            match parse_crate_name(entry_path) {
                Ok(name) => {
                    let islib = match is_crate_lib(
                        entry_path
                            .to_str()
                            .unwrap()
                            .strip_suffix("Cargo.toml")
                            .unwrap(),
                    ) {
                        Ok(islib) => islib,
                        Err(e) => {
                            error!("parse error: {}", e);
                            continue;
                        }
                    };

                    debug!("Found Crate: {}, islib: {}", name, islib);
                    let program =
                        from_cargo_toml(local_repo_path.clone(), entry_path.to_path_buf(), &id)
                            .unwrap();

                    let uprogram = if islib {
                        UProgram::Library(Library::new(&id.to_string(), &name, -1, None))
                    } else {
                        UProgram::Application(Application::new(id.to_string(), &name))
                    };

                    let has_type = HasType {
                        SRC_ID: program.id.clone(),
                        DST_ID: program.id.clone(),
                    };

                    debug!(
                        "program: {:?}, has_type: {:?}, uprogram: {:?}",
                        program, has_type, uprogram
                    );
                    insert_program_by_name(name.clone(), (program.clone(), uprogram.clone()));

                    res.push((program, has_type, uprogram));
                }
                Err(e) => error!("Error parsing name {}: {}", entry_path.display(), e),
            }
        }
    }

    res
}

// 解析Cargo.toml文件来确定crate的名称和是否为库
fn parse_crate_name(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let value = content.parse::<Value>()?;

    // a package name, no matter lib or bin
    let package_name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or("Failed to find package name")?
        .to_owned();

    Ok(package_name)
}

fn is_crate_lib(crate_path: &str) -> Result<bool, String> {
    let metadata = MetadataCommand::new()
        .manifest_path(PathBuf::from(crate_path).join("Cargo.toml"))
        .exec()
        .map_err(|e| format!("{:#?}", e))?;

    // 遍历所有包
    let package = metadata.root_package().unwrap();
    // 遍历该包的所有目标 (libraries, binaries, examples, etc.)
    for target in &package.targets {
        let target_types: Vec<_> = target.kind.to_vec();

        if target_types.contains(&"bin".to_string()) {
            //println!("{} is a binary crate.", package.name);
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn from_cargo_toml(
    local_repo_path: PathBuf,
    cargo_toml_path: PathBuf,
    id: &str,
) -> Result<Program, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(cargo_toml_path)?;
    let parsed = content.parse::<Value>()?;

    let program = Program::new(
        id.to_string(),
        parsed["package"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        parsed["package"]
            .get("decription")
            .unwrap_or(&Value::String(String::default()))
            .as_str()
            .map(String::from),
        get_namespace_by_repo_path(local_repo_path.to_str().unwrap()),
        parsed["package"]["version"].as_str().map(String::from),
        None,
        None,
        None,
    );

    Ok(program)
}

fn get_fields<T: Serialize>(item: &T) -> Vec<String> {
    let mut fields = Vec::new();
    let json = json!(item);
    if let serde_json::Value::Object(map) = json {
        fields = map
            .values()
            .map(|value| {
                match value {
                    serde_json::Value::String(s) => s.clone(), // 直接使用字符串值。
                    // 对于其他类型，使用`to_string`，这将丢弃原始serde_json的编码方式。
                    _ => value.to_string().trim_matches('"').to_owned(),
                }
            })
            .collect::<Vec<_>>();
    }
    fields
}

pub(crate) fn write_into_csv<T: Serialize + Default + Debug>(
    csv_path: PathBuf,
    programs: Vec<T>,
) -> Result<(), Box<dyn Error>> {
    // open the csv

    let serialized = serde_json::to_value(&T::default()).unwrap();

    // 将JSON值转换为对象并提取字段名
    if let serde_json::Value::Object(map) = serialized {
        //let field_names: Vec<String> = map.keys().cloned().collect();
        let field_names: Vec<&str> = map.keys().map(|s| s.as_str()).collect();

        debug!("{:?}", field_names);

        write_to_csv(field_names, csv_path.to_str().unwrap(), false)?;
    }

    for program in &programs {
        let fields = get_fields(program);
        let fields = fields.iter().map(|s| s.as_str()).collect::<Vec<_>>();

        debug!("{:?}", fields);
        write_to_csv(fields, csv_path.to_str().unwrap(), true)?;
    }

    Ok(())
}

fn write_to_csv(data: Vec<&str>, file_path: &str, append: bool) -> Result<(), Box<dyn Error>> {
    let file = if append {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?
    } else {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)?
    };

    let mut wtr = Writer::from_writer(file);

    wtr.write_record(&data)?;

    wtr.flush()?;
    Ok(())
}

/// An auxiliary function
///
/// Extracts namespace e.g. "tokio-rs/tokio" from the git url https://www.github.com/tokio-rs/tokio
pub(crate) fn extract_namespace(url_str: &str) -> Result<String, String> {
    /// auxiliary function
    fn remove_dot_git_suffix(input: &str) -> String {
        if input.ends_with(".git") {
            input.replace(".git", "")
        } else {
            input.to_string()
        }
    }

    let url = Url::parse(url_str).map_err(|e| format!("Failed to parse URL {}: {}", url_str, e))?;

    // /tokio-rs/tokio
    let path_segments = url
        .path_segments()
        .ok_or("Cannot extract path segments from URL")?;

    let segments: Vec<&str> = path_segments.collect();

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
    Ok(remove_dot_git_suffix(&namespace))
}
