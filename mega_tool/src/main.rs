use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    str::FromStr,
};

use clap::Parser;
use csv::ReaderBuilder;
use url::Url;

use mega_tool::{
    command::{Cli, Commands},
    crate_to_repo::convert_crate_to_repo,
    handle_repo::add_and_push_to_remote,
    incremental_update::incremental_update,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let args = Cli::parse();
    match args.command {
        Commands::Upload => {
            add_and_push_to_remote(args.workspace).await;
        }
        Commands::Crate => {
            convert_crate_to_repo(args.workspace).await;
        }
        Commands::Incremental => {
            incremental_update().await;
        }
    }
}

pub fn convert_script() {
    let work_dir = PathBuf::from("/media/parallels/Lexar/GitHub");
    let clone_script = work_dir.join("github.sh");
    let reader = BufReader::new(File::open(clone_script).unwrap());
    let clone_script_new = work_dir.join("github_0826.sh");
    let mut new_script = File::create(clone_script_new).unwrap();
    for line in reader.lines() {
        let url = Url::parse(&line.unwrap().replace("git clone ", "")).unwrap();
        let strs: Vec<&str> = url.path().split('/').collect();
        let username = strs[1];
        let reponame = strs[2];
        let combine = format!("{}/{}", username, reponame);
        // mkdir -p trustwallet && git clone https://github.com/trustwallet/assets ./trustwallet
        let line = format!(
            "mkdir -p {} && git clone https://github.com/{} ./{}",
            combine, combine, combine
        );
        new_script.write_all(line.as_bytes()).unwrap();
        new_script.write_all(b"\n").unwrap();
    }
}

pub fn move_file_0826_github() {
    // let work_dir = PathBuf::from("/Users/yetianxing/workdir/");
    let work_dir = PathBuf::from("/media/parallels/Lexar/Gitee");
    let temp = work_dir.join("temp");
    if !temp.exists() {
        fs::create_dir(&temp).unwrap();
    }
    let clone_script = work_dir.join("gitee.sh");

    let reader = BufReader::new(File::open(clone_script).unwrap());
    for line in reader.lines() {
        let url = Url::parse(&line.unwrap().replace("git clone ", "")).unwrap();
        let strs: Vec<&str> = url.path().split('/').collect();
        let username = strs[1];
        let reponame = strs[2];
        let current_name = work_dir.join(reponame);
        let target_name = work_dir.join(username).join(reponame);
        let temp_name = temp.join(reponame);

        if current_name.exists() && current_name.is_dir() && !target_name.exists() {
            println!("{:?}, {:?}, {:?}", current_name, temp_name, target_name);
            std::fs::rename(&current_name, &temp_name).unwrap();
            std::fs::create_dir_all(target_name.clone()).unwrap();
            std::fs::rename(&temp_name, &target_name).unwrap();
        }
    }
    //remove tmep
    fs::remove_dir_all(temp).unwrap();
}

pub fn convert_origin() {
    let file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/gha_repo_list_top_100000.csv").unwrap();
    let github_file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/all_repositories_github.log").unwrap();
    let gitee_file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/all_repositories_gitee.log").unwrap();
    let file = File::open(file_path).unwrap();
    let mut github_file = File::create(github_file_path).unwrap();
    let mut gitee_file = File::create(gitee_file_path).unwrap();
    // Create a CSV reader
    let mut rdr = ReaderBuilder::new().from_reader(file);
    // Iterate over the CSV records
    for result in rdr.records() {
        // Unwrap the record or handle the error
        let record = result.unwrap();

        let git_url = record.get(0).unwrap_or("");
        if !git_url.is_empty() {
            println!("Field 1: {}", git_url);
            let url = "git clone ".to_owned() + git_url;
            if git_url.contains("github.com") {
                github_file.write_all(url.as_bytes()).unwrap();
                github_file.write_all(b"\n").unwrap();
            } else {
                gitee_file.write_all(url.as_bytes()).unwrap();
                gitee_file.write_all(b"\n").unwrap();
            }
        }
    }
}

pub fn convert0817() {
    let file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/gha_repo_list_top_100000.csv").unwrap();
    let github_file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/gha_repo_list_top_100000_output.sh")
            .unwrap();

    let file = File::open(file_path).unwrap();
    let mut output = File::create(github_file_path).unwrap();
    // Create a CSV reader
    let mut rdr = ReaderBuilder::new().from_reader(file);
    // Iterate over the CSV records
    for result in rdr.records() {
        // Unwrap the record or handle the error
        let record = result.unwrap();

        let owner = record.get(0).unwrap_or("");
        let git_url = record.get(2).unwrap_or("");
        if !git_url.is_empty() && !owner.is_empty() {
            println!("Field 1: {}", git_url);
            let command = format!("mkdir -p {} && git clone {} ./{}", owner, git_url, owner);
            output.write_all(command.as_bytes()).unwrap();
            output.write_all(b"\n").unwrap();
        }
    }
}

pub fn convert_cratesio_csv() {
    let file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/2023-10-07-020047/data/crates.csv").unwrap();
    let github_file_path =
        PathBuf::from_str("/Users/yetianxing/Downloads/crates_repo_output.log").unwrap();

    let file = File::open(file_path).unwrap();
    let mut output = File::create(github_file_path).unwrap();
    // Create a CSV reader
    let mut rdr = ReaderBuilder::new().from_reader(file);
    // Iterate over the CSV records
    for result in rdr.records() {
        // Unwrap the record or handle the error
        let record = result.unwrap();

        // let owner = record.get(0).unwrap_or("");
        let git_url = record.get(9).unwrap_or("");
        if !git_url.is_empty() && git_url.contains("github.com") {
            println!("Field 1: {}", git_url);
            let url = Url::parse(git_url).expect("Failed to parse URL");
            // Get the path segments
            let path_segments: Vec<&str> = url.path_segments().unwrap().collect();
            let owner = path_segments[0];
            let command = format!("{},{}", owner, git_url);
            output.write_all(command.as_bytes()).unwrap();
            output.write_all(b"\n").unwrap();
        }
    }
}
