use crate::utils::{get_namespace_by_repo_path, insert_program_by_name};
use model::tugraph_model::{Application, HasType, Library, Program, UProgram};
use std::{
    fs,
    path::{Path, PathBuf},
};
use toml::Value;
use uuid::Uuid;
use walkdir::WalkDir;

// Given a project path, parse the metadata
pub(crate) async fn extract_info_local(
    local_repo_path: PathBuf,
    git_url: String,
) -> Vec<(Program, HasType, UProgram)> {
    let mut res = vec![];

    // walk the directories of the project
    for entry in WalkDir::new(local_repo_path.clone())
        .into_iter()
        .filter_map(|x| x.ok())
    {
        let entry_path = entry.path();

        // if entry is Cargo.toml, ...
        if entry_path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
            tracing::trace!("entry_path: {:?}", entry_path);
            let crate_name_result = parse_crate_name(entry_path).await;
            match crate_name_result {
                Ok(name) => {
                    tracing::trace!("package name: {}", name);
                    let islib_result = is_crate_lib(
                        entry_path
                            .to_str()
                            .unwrap()
                            .strip_suffix("Cargo.toml")
                            .unwrap(),
                    )
                    .await;
                    let islib = match islib_result {
                        Ok(islib) => islib,
                        Err(e) => {
                            tracing::error!("parse error: {}", e);
                            continue;
                        }
                    };

                    tracing::debug!("Found Crate: {}, islib: {}", name, islib);
                    let id = Uuid::new_v4().to_string();
                    let mut program =
                        from_cargo_toml(local_repo_path.clone(), entry_path.to_path_buf(), &id)
                            .await
                            .unwrap();
                    program.mega_url = Some(git_url.clone());

                    let uprogram = if islib {
                        UProgram::Library(Library::new(&id.to_string(), &name, -1, None))
                    } else {
                        UProgram::Application(Application::new(id.to_string(), &name))
                    };

                    let has_type = HasType {
                        SRC_ID: program.id.clone(),
                        DST_ID: program.id.clone(),
                    };

                    tracing::trace!(
                        "program: {:?}, has_type: {:?}, uprogram: {:?}",
                        program,
                        has_type,
                        uprogram
                    );
                    insert_program_by_name(name.clone(), (program.clone(), uprogram.clone()));

                    res.push((program, has_type, uprogram));
                }
                Err(e) => tracing::warn!("Error parsing name {}: {}", entry_path.display(), e),
            }
        }
    }

    res
}

async fn parse_crate_name(path: &Path) -> Result<String, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value = content.parse::<Value>().map_err(|e| e.to_string())?;

    // a package name, no matter lib or bin
    let package_name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or("Failed to find package name, it is a workspace")?
        .to_owned();

    Ok(package_name)
}

async fn is_crate_lib(crate_path: &str) -> Result<bool, String> {
    let cargo_toml_path = Path::new(crate_path).join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(cargo_toml_path)
        .map_err(|e| format!("Failed to read Cargo.toml: {}", e))?;

    let cargo_toml: Value = cargo_toml_content
        .parse::<Value>()
        .map_err(|e| format!("Failed to parse Cargo.toml: {}", e))?;

    // 优先检查 Cargo.toml 中的 '[lib]' 和 '[[bin]]'
    let has_lib_in_toml = cargo_toml.get("lib").is_some();
    let has_bin_in_toml = cargo_toml.get("bin").map_or(false, |bins| {
        bins.as_array().map_or(false, |b| !b.is_empty())
    });

    if has_lib_in_toml || has_bin_in_toml {
        return Ok(has_lib_in_toml && !has_bin_in_toml);
    }

    // 如果 Cargo.toml 中无明显标识，退回到检查文件
    let lib_rs_exists = Path::new(crate_path).join("src/lib.rs").exists();
    let main_rs_exists = Path::new(crate_path).join("src/main.rs").exists();

    // 如果 'src/lib.rs' 存在，且 'src/main.rs' 不存在，更可能是库
    if lib_rs_exists && !main_rs_exists {
        return Ok(true);
    }

    // 如果存在 'src/main.rs'，则倾向于不是库
    if main_rs_exists {
        return Ok(false);
    }

    // 如果没有明显的线索，回退为默认假设不是库
    Ok(false)
}

async fn from_cargo_toml(
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
            .unwrap_or(&Value::String(String::from("None")))
            .as_str()
            .map(String::from),
        get_namespace_by_repo_path(local_repo_path.to_str().unwrap()),
        None,
        None,
        None,
        None,
    );

    Ok(program)
}
