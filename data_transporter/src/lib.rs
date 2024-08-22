mod data_packer;
mod data_reader;
mod db;
mod transporter;

use model::tugraph_model::UVersion;
pub use transporter::Transporter;

#[derive(Debug)]
pub struct NameVersion {
    pub name: String,
    pub version: String,
}

impl NameVersion {
    // 解析 "name/version" 格式的字符串
    pub fn from_string(name_version: &str) -> Option<Self> {
        let parts: Vec<&str> = name_version.split('/').collect();
        if parts.len() == 2 {
            Some(NameVersion {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
            })
        } else {
            None
        }
    }
}

pub struct VersionInfo {
    pub version_base: UVersion,
    pub dependencies: Vec<NameVersion>,
}
