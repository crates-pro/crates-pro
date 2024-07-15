use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Version {
    pub name: String,
    pub version: String,
}

impl Version {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
        }
    }
}
