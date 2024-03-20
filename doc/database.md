# Database Design for crates.pro 

## Graph Model Design for TuGraph

### Storage Structure Design Goals

Our aim is to create a graph database model that effectively represents the dependency relationships between crates and their versions within Rust's package management system in TuGraph. This model is designed to tackle two core issues:
- **Storage**: Represent crates and their version information. This is achieved by distinguishing between crate metadata and specific version information through two types of nodes (CrateMaster and CrateVersion).
- **Computation**: Track dependencies. Dependencies between different crate versions are represented using edges, allowing for the tracking and querying of whether a version of a crate directly or indirectly depends on a specific version of another crate.


### Edge Design

- **has_version**:
  - Direction: From CrateMaster to CrateVersion.
  - Description: Represents the version nodes owned by a crate master node.
  - Attributes may be none or only include version release sequence information (for easy traversal).
- **depends_on**:
  - Direction: From one CrateVersion to another CrateVersion.
  - Description: Represents that a version of a crate depends on a specific version of another crate.
  - Attributes:
    - dependency_type: String, the type of dependency, e.g., dev, build, regular.
    - optional: Boolean, whether this dependency is optional.
    - default_features: Boolean, whether the default features of the dependency are used.
    - features: String Array, a list of activated features.
- **updated_to**:
  - Direction: From an older version to a newer version.
  - Description:
    - By traversing the updated_to edges, we can efficiently query the complete update history of any version of a crate.
    - The newest version can be quickly located by finding the CrateVersion node that does not have an updated_to outgoing edge, which is more direct and efficient than comparing dates or version number strings.
    - When a new version is released, it only requires adding a new CrateVersion node and establishing an updated_to edge towards it. This change can trigger a notification mechanism to inform all users subscribed to updates from the old version.
  - Attributes:
    - updated_date: The date of the update, recording the release date of the new version
    - change_log: Optional, records the main changes summary from the previous version to the current version

---

## ER Diagram for PostgreSQL

### Table Overview

| Table Name    | Description                                                              |
|---------------|--------------------------------------------------------------------------|
| crate         | Crate from Rust community registry - crates.io                           |
| crate_version | Crate published specification version                                    |
| info          | Crate general information                                                |
| application   | Application developed by Rust                                            |
| app_version   | Application tags                                                         |
| security      | Security Advisories                                                      |
| sensleak      | Sensitive data, specifically targeting sensitive information within code |
| productivity  | Productivity data from oss-compass                                       |
| robustness    | Robustness data from oss-compass                                         |
| performance   | Performance test data                                                    |

### ER Diagram

```mermaid
erDiagram
    
    CRATE ||--o| CRATE_VERSION : has_version
    CRATE_VERSION ||--o| INFO : depends_on
    CRATE_VERSION ||--o{ CRATE_VERSION : dependencies
    CRATE_VERSION ||--o{ CRATE_VERSION : dev_dependencies
    CRATE_VERSION ||--o{ SECURITY : has
    CRATE_VERSION  ||--o| PRODUCTIVITY : productivity
    CRATE_VERSION  ||--o| ROBUSTNESS : robustness
    CRATE_VERSION  ||--o| PERFORMANCE : performance
    CRATE_VERSION  ||--o| SENSLEAK : has
    APPLICATION ||--o| APP_VERSION : depends_on
    APP_VERSION ||--o{ CRATE_VERSION : dependencies
    APP_VERSION ||--o{ SECURITY : has
    APP_VERSION ||--o| PRODUCTIVITY : productivity
    APP_VERSION  ||--o| ROBUSTNESS : robustness
    APP_VERSION  ||--o| PERFORMANCE : performance
    APP_VERSION  ||--o| SENSLEAK : has
```

### Table Details

#### crate

| Column        | Type         | Constraints | Description                                         |
|---------------|--------------|-------------|-----------------------------------------------------|
| id            | BIGINT       | PRIMARY KEY |                                                     |
| name          | VARCHAR(255) | NOT NULL    |                                                     |
| namespace     | VARCHAR(255) | NULL        |                                                     |
| repository    | TEXT         | NULL        | The git repository url from GitHub or other service |
| documentation | TEXT         | NULL        | The documentation url                               |
| categories    | TEXT         | NULL        |                                                     |
| keywords      | TEXT         | NULL        |                                                     |
| download      | BIGINT       | NOT NULL    | The download count from crates.io                   |
| registry      | VARCHAR(255) | NOT NULL    | Default registry is crates.io                       |
| created_at    | TIMESTAMP    | NOT NULL    |                                                     |
| updated_at    | TIMESTAMP    | NOT NULL    |                                                     |

#### crate_version

| Column     | Type         | Constraints | Description                          |
|------------|--------------|-------------|--------------------------------------|
| id         | BIGINT       | PRIMARY KEY |                                      |
| crate_id   | BIGINT       | NOT NULL    |                                      |
| version    | VARCHAR(255) | NOT NULL    |                                      |
| license    | VARCHAR(255) | NOT NULL    |                                      |
| sloc       | BIGINT       | NOT NULL    | Source lines of code                 |
| dep_sloc   | BIGINT       | NOT NULL    | Source lines of code of dependencies |
| features   | TEXT         | NULL        | Cargo features                       |
| sbom       | TEXT         | NULL        | Software Bill of Materials           |
| created_at | TIMESTAMP    | NOT NULL    |                                      |
| updated_at | TIMESTAMP    | NOT NULL    |                                      |

#### info

| Column        | Type         | Constraints | Description |
|---------------|--------------|-------------|-------------|
| id            | BIGINT       | PRIMARY KEY |             |
| version_id    | BIGINT       | NOT NULL    |             |
| edition       | VARCHAR(255) | NULL        |             |
| description   | TEXT         | NULL        |             |
| authors       | TEXT         | NULL        |             |
| size          | BIGINT       | NOT NULL    |             |
| created_at    | TIMESTAMP    | NOT NULL    |             |
| updated_at    | TIMESTAMP    | NOT NULL    |             |

#### application

| Column        | Type         | Constraints | Description                                         |
|---------------|--------------|-------------|-----------------------------------------------------|
| id            | BIGINT       | PRIMARY KEY |                                                     |
| repository    | TEXT         | NULL        | The git repository url from GitHub or other service |
| documentation | TEXT         | NULL        | The documentation url                               |
| name          | VARCHAR(255) | NOT NULL    |                                                     |
| created_at    | TIMESTAMP    | NOT NULL    |                                                     |
| updated_at    | TIMESTAMP    | NOT NULL    |                                                     |

#### app_version

| Column     | Type      | Constraints | Description |
|------------|-----------|-------------|-------------|
| id         | BIGINT    | PRIMARY KEY |             |
| app_id     | BIGINT    | NOT NULL    |             |
| created_at | TIMESTAMP | NOT NULL    |             |
| updated_at | TIMESTAMP | NOT NULL    |             |

#### security

| Column     | Type      | Constraints | Description |
|------------|-----------|-------------|-------------|
| id         | BIGINT    | PRIMARY KEY |             |
| app_id     | BIGINT    | NULL        |             |
| version_id | BIGINT    | NULL        |             |
| created_at | TIMESTAMP | NOT NULL    |             |
| updated_at | TIMESTAMP | NOT NULL    |             |

#### productivity

| Column       | Type       | Constraints | Description |
|--------------|------------|-------------|-------------|
| id           | BIGINT     | PRIMARY KEY |             |
| app_id       | BIGINT     | NULL        |             |
| version_id   | BIGINT     | NULL        |             |
| created_at   | TIMESTAMP  | NOT NULL    |             |
| updated_at   | TIMESTAMP  | NOT NULL    |             |

#### robustness

| Column     | Type      | Constraints | Description |
|------------|-----------|-------------|-------------|
| id         | BIGINT    | PRIMARY KEY |             |
| app_id     | BIGINT    | NULL        |             |
| version_id | BIGINT    | NULL        |             |
| created_at | TIMESTAMP | NOT NULL    |             |
| updated_at | TIMESTAMP | NOT NULL    |             |

#### performance

| Column     | Type      | Constraints | Description |
|------------|-----------|-------------|-------------|
| id         | BIGINT    | PRIMARY KEY |             |
| app_id     | BIGINT    | NULL        |             |
| version_id | BIGINT    | NULL        |             |
| created_at | TIMESTAMP | NOT NULL    |             |
| updated_at | TIMESTAMP | NOT NULL    |             |

#### sensleak

| Column     | Type      | Constraints | Description |
|------------|-----------|-------------|-------------|
| id         | BIGINT    | PRIMARY KEY |             |
| app_id     | BIGINT    | NULL        |             |
| version_id | BIGINT    | NULL        |             |
| created_at | TIMESTAMP | NOT NULL    |             |
| updated_at | TIMESTAMP | NOT NULL    |             |


### Usage

#### Steps

1. open dev-containers and wait for compiling
2. cd ../../workspace/
3. In bash, you can input `/build/output/lgraph_server -c /build/output/lgraph_daemon.json -d restart` to start TuGraph Server.
    - run `netstat -tuln | grep -E '7687|7070'` to check if it successes. The terminal will show 
      ```
      tcp        0      0 0.0.0.0:7687            0.0.0.0:*               LISTEN     
      tcp        0      0 0.0.0.0:7070            0.0.0.0:*               LISTEN 
      ```
    - run `cargo test test_tugraph_setup` to test it.
4. Then, you can code and test it.
    - The bolt port is 7687, and HTTP port is 7070
    - Open http://localhost:7070 in your browser. The username is `admin`, and the password is `73@TuGraph`.
5. `cargo test` to run all the tests.