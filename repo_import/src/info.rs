use cargo_metadata::MetadataCommand;
use csv::Writer;
use model::crate_info::{Application, Library, Program, UProgram};
use serde::Serialize;
use serde_json::{json, Value};
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) fn extract_info_local(local_repo_path: PathBuf) -> (Program, UProgram) {
    let manifest_path = local_repo_path.join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .unwrap();

    let id = Uuid::new_v4();

    let islib = if metadata.packages.len() == 1 {
        let package = &metadata.packages[0];
        package
            .targets
            .iter()
            .any(|target| target.kind.contains(&"lib".to_string()))
    } else {
        todo!();
    };

    let package = metadata.packages.first().unwrap();
    let program = Program::new(
        id.clone().to_string(),
        package.name.clone(),
        package.description.clone(),
        Some(format!(
            "{}/{}",
            metadata.workspace_root.as_str(),
            package.name
        )),
        Some(package.version.to_string()),
        package.repository.clone(),
        None, // There's no "mega_url" field in the Cargo.toml
        package.documentation.clone(),
    );

    let uprogram = if islib {
        UProgram::Library(Library::new(
            &id.to_string(),
            &package.name,
            -1,
            package.homepage.as_deref(),
        ))
    } else {
        UProgram::Application(Application::new(id.to_string(), &package.name))
    };

    (program, uprogram)
}

pub(crate) fn write_into_csv<T: Serialize>(
    csv_path: PathBuf,
    programs: Vec<T>,
) -> Result<(), Box<dyn Error>> {
    // open the csv
    let mut file = File::create(csv_path)?;

    // header
    let header = format!(
        "{},{},{}",
        std::any::type_name::<&str>(),
        std::any::type_name::<&str>(),
        std::any::type_name::<&str>()
    );
    file.write_all(header.as_bytes())?;
    file.write_all(b"\n")?;

    drop(file);

    let mut writer = Writer::from_path("output.csv")?;

    for program in &programs {
        let fields = get_fields(program);
        writer.write_record(&fields)?;
    }

    Ok(())
}

fn get_fields<T: Serialize>(item: &T) -> Vec<String> {
    let mut fields = Vec::new();

    let json = json!(item);

    if let Value::Object(map) = json {
        for (_key, value) in map {
            fields.push(value.to_string());
        }
    }

    fields
}
