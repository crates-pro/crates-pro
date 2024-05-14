use crate::utils::{get_namespace_by_repo_path, insert_program_by_name};
use cargo_metadata::MetadataCommand;
use model::crate_info::{Application, HasType, Library, Program, UProgram};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml::Value;
use uuid::Uuid;
use walkdir::WalkDir;

// Given a project path, parse the metadata
pub(crate) fn extract_info_local(local_repo_path: PathBuf) -> Vec<(Program, HasType, UProgram)> {
    trace!("Parse repo {:?}", local_repo_path);
    let mut res = vec![];

    let id = Uuid::new_v4().to_string();

    // walk the directories of the project
    for entry in WalkDir::new(local_repo_path.clone())
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

    let package = metadata.root_package().unwrap();
    for target in &package.targets {
        let target_types: Vec<_> = target.kind.to_vec();

        if target_types.contains(&"bin".to_string()) {
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
