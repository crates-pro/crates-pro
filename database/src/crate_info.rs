use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Represents detailed information about a Rust crate.
///
/// This structure includes metadata fields that describe a crate, such as its name,
/// current version, description, and various URLs related to its documentation,
/// repository, and license, along with a count of its dependencies.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CrateInfo {
    /// The name of the crate.
    name: String,
    /// The current version of the crate.
    version: String,
    /// An optional description of the crate.
    description: Option<String>,
    /// An optional URL pointing to the crate's documentation.
    documentation_url: Option<String>,
    /// An optional URL pointing to the crate's source code repository.
    repository_url: Option<String>,
    /// An optional string indicating the license under which the crate is distributed.
    license: Option<String>,
    /// The number of dependencies this crate has.
    dependencies_count: usize,
}

impl CrateInfo {
    /// Constructs a new `CrateInfo`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the crate.
    /// * `version` - The current version of the crate.
    /// * `description` - An optional description of the crate.
    /// * `documentation_url` - An optional URL for the crate's documentation.
    /// * `repository_url` - An optional URL for the crate's source code repository.
    /// * `license` - An optional license string.
    /// * `dependencies_count` - The number of dependencies of the crate.
    pub fn new(
        name: String,
        version: String,
        description: Option<String>,
        documentation_url: Option<String>,
        repository_url: Option<String>,
        license: Option<String>,
        dependencies_count: usize,
    ) -> Self {
        CrateInfo {
            name,
            version,
            description,
            documentation_url,
            repository_url,
            license,
            dependencies_count,
        }
    }
}

/// Represents a specific version of a Rust crate, including its
/// release date and (optionally) a description.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CrateVersion {
    /// The version string.
    version: String,
    /// The release date of this version.
    release_date: NaiveDate,
    /// An optional description of this version.
    description: Option<String>,
}

impl CrateVersion {
    /// Constructs a new `CrateVersion`.
    ///
    /// # Arguments
    ///
    /// * `version` - The version string of the crate.
    /// * `release_date` - The `NaiveDate` this version was released.
    /// * `description` - An optional description of what this version introduces or changes.
    pub fn new(version: String, release_date: NaiveDate, description: Option<String>) -> Self {
        CrateVersion {
            version,
            release_date,
            description,
        }
    }
}
