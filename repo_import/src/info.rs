use cargo_metadata::MetadataCommand;
use csv::Writer;
use model::crate_info::{Application, Library, Program, UProgram};
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use toml::Value;
use uuid::Uuid;
use walkdir::WalkDir;

// Given a project path, parse the metadata
pub(crate) fn extract_info_local(local_repo_path: PathBuf) -> Vec<(Program, UProgram)> {
    trace!("Parse repo {:?}", local_repo_path);
    let mut res = vec![];

    let id = Uuid::new_v4().to_string();

    // It is possible that there is no Cargo.toml file in the project root directory,
    // so the root directories are one level down
    let (min_depth, max_depth) = if exists_cargo_toml(&local_repo_path) {
        (1, 2)
    } else {
        (2, 3)
    };

    // walk the directories of the project
    for entry in WalkDir::new(local_repo_path)
        .min_depth(min_depth) // owner/proj/Cargo.toml
        .max_depth(max_depth) // workspace: owner/proj/Cargo.toml
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
                    let program = from_cargo_toml(entry_path, &id).unwrap();

                    let uprogram = if islib {
                        UProgram::Library(Library::new(&id.to_string(), &name, -1, None))
                    } else {
                        UProgram::Application(Application::new(id.to_string(), &name))
                    };

                    debug!("program: {:?}, uprogram: {:?}", program, uprogram);

                    res.push((program, uprogram));
                }
                Err(e) => error!("Error parsing name {}: {}", entry_path.display(), e),
            }
        }
    }

    res
}

fn exists_cargo_toml(path: &Path) -> bool {
    let cargo_toml_path = path.join("Cargo.toml");
    cargo_toml_path.is_file()
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
    // 获取当前项目的 cargo 元数据
    let metadata = MetadataCommand::new()
        .manifest_path(PathBuf::from(crate_path).join("Cargo.toml"))
        .exec()
        .map_err(|e| format!("{:#?}", e))?;

    // 遍历所有包
    let package = metadata.root_package().unwrap();
    // 遍历该包的所有目标 (libraries, binaries, examples, etc.)
    for target in &package.targets {
        let target_types: Vec<_> = target.kind.to_vec();

        debug!(
            "Package Name: {} - Target: {} - Types: {:?}",
            package.name, target.name, target_types
        );

        // 判断当前target是否是 lib 或 bin
        // 注意：一个包可以同时包含多个类型的目标
        // if target_types.contains(&"lib".to_string()) {
        //     println!("{} is a library crate.", package.name);
        // }
        if target_types.contains(&"bin".to_string()) {
            println!("{} is a binary crate.", package.name);
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn from_cargo_toml<P: AsRef<Path>>(
    path: P,
    id: &str,
) -> Result<Program, Box<dyn std::error::Error>> {
    // 读取Cargo.toml文件内容
    let content = fs::read_to_string(path)?;
    // 解析TOML内容到toml::Value
    let parsed = content.parse::<Value>()?;

    // 解析并构造Program实例，这里简化处理，实际情况可能需要更复杂的逻辑来提取和处理信息
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
        None, // 通常Cargo.toml中不包含namespace信息，可能需要其他途径获取
        parsed["package"]["version"].as_str().map(String::from),
        None, // 需要从其他地方获取
        None, // 需要从其他地方获取
        None, // 需要从其他地方获取
    );

    Ok(program)
}

pub(crate) fn write_into_csv<T: Serialize + Default>(
    csv_path: PathBuf,
    programs: Vec<T>,
) -> Result<(), Box<dyn Error>> {
    // open the csv

    let serialized = serde_json::to_value(&T::default())?;

    // 将JSON值转换为对象并提取字段名
    if let serde_json::Value::Object(map) = serialized {
        //let field_names: Vec<String> = map.keys().cloned().collect();
        let field_names: Vec<&str> = map.keys().map(|s| s.as_str()).collect();

        debug!("{:?}", field_names);

        write_to_csv(field_names, csv_path.to_str().unwrap())?;
    }

    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(csv_path.clone())?;
    let mut wtr = Writer::from_writer(file);

    for program in &programs {
        let fields = get_fields(program);
        wtr.write_record(&fields)?;
    }

    Ok(())
}

fn get_fields<T: Serialize>(item: &T) -> Vec<String> {
    let mut fields = Vec::new();

    let json = json!(item);

    if let serde_json::Value::Object(map) = json {
        for (_key, value) in map {
            fields.push(value.to_string());
        }
    }

    fields
}

fn write_to_csv(data: Vec<&str>, file_path: &str) -> Result<(), Box<dyn Error>> {
    // 打开文件准备写入
    let mut wtr = Writer::from_path(file_path)?;

    // 将data作为单独的记录写入
    wtr.write_record(&data)?;

    // 确保所有内容都被刷新到文件
    wtr.flush()?;
    Ok(())
}
