# Crates Pro System Architecture Document

## Table of Contents

- [1. System Module Overview](#1-system-module-overview)
- [2. Core Module Details](#2-core-module-details)
  - [2.1 crates_pro Module](#21-crates_pro-module)
  - [2.2 data_transporter Module](#22-data_transporter-module)
  - [2.3 model Module](#23-model-module)
  - [2.4 repo_import Module](#24-repo_import-module)
  - [2.5 search Module](#25-search-module)
  - [2.6 sync_tool Module](#26-sync_tool-module)
- [3. Backend Development Guide](#3-backend-development-guide)

## 1. System Module Overview

Crates Pro consists of multiple collaborating modules, each responsible for specific functions:

| Module Name | Main Function | Development Status |
|-------------|---------------|-------------------|
| crates_pro | System entry point, task scheduling | Logic complete, no additional features needed in the short term |
| data_transporter | Bridge between frontend and backend, provides API | Continuous development needed to support frontend requirements |
| model | Data structure definitions | May need expansion as the system evolves |
| repo_import | Data import and parsing | Parsing logic complete, may need additional analysis features |
| search | Crate search functionality | Large models could be introduced to optimize ranking strategies |
| sync_tool | Kafka incremental message generation | Independently deployed module |

## 2. Core Module Details

### 2.1 crates_pro Module

**Functionality**: Project entry point, responsible for starting and coordinating various asynchronous tasks

**Core Components**:
- `main.rs`: Program entry point, calls `core_controller.run()` to start the system
- `core_controller.rs`: Defines three key asynchronous tasks

**Asynchronous Tasks**:

| Task Name | Environment Variable Control | Function Description |
|-----------|------------------------------|---------------------|
| import | CRATES_PRO_IMPORT | Reads Kafka message queue, parses crate information |
| analysis | CRATES_PRO_ANALYSIS | Performs analysis on crates |
| package | CRATES_PRO_PACKAGE | Packages data to PG database, connects to frontend when value is 1 |

When `CRATES_PRO_PACKAGE=1`, the system calls `run_api_server()` to connect to the frontend and starts the API service.

### 2.2 data_transporter Module

**Functionality**: Bridge between frontend and backend, API provider

**Core Components**:
- `lib.rs`: Module entry point, defines API routes
- `handler.rs`: API implementation logic
- `data_reader.rs`: TuGraph data reading
- `db.rs`: PostgreSQL data reading
- `data_packer.rs`: Data import to PostgreSQL

**Main API Endpoints**:

| Endpoint Path | Function Description |
|---------------|---------------------|
| `/api/search` | Crate search, sorted by name relevance |
| `/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}` | Get specific crate information |
| `/api/crates/{...}/{...}/{...}/{...}/dependencies` | Get dependency information |
| `/api/crates/{...}/{...}/{...}/{...}/dependencies/graph` | Get dependency tree graph |
| `/api/crates/{...}/{...}/{...}/{...}/dependents` | Get dependent information |
| `/api/crates/{...}/{...}/{...}/{...}/versions` | Get historical version information |

**Endpoint Details**:

1. **Search Endpoint** (`/api/search`)
   - Accepts frontend query parameters
   - Calls search module's search and ranking API
   - Searches for crates by name from PG database and sorts them
   - Returns results in JSON format

2. **Crate Information Endpoint** (`/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}`)
   - Parameters: nsfront and nsbehind represent namespace front and back parts, cratename is the crate name, version is the version
   - Uses parameters to query parsed crate information from TuGraph
   - Returns data including CVE information, dependency relationships, etc.

3. **Dependency Information Endpoint** (`/api/crates/{...}/{...}/{...}/{...}/dependencies`)
   - Gets dependency information for a crate
   - Includes direct dependencies (declared in Cargo.toml) and indirect dependencies
   - Queries from TuGraph and returns JSON results

4. **Dependency Graph Endpoint** (`/api/crates/{...}/{...}/{...}/{...}/dependencies/graph`)
   - Gets dependency information and builds a dependency tree
   - Returns dependency tree in JSON format
   - Frontend displays as a dependency graph using visualization tools

5. **Dependents Information Endpoint** (`/api/crates/{...}/{...}/{...}/{...}/dependents`)
   - Gets which crates depend on the current crate
   - Includes direct and indirect dependent relationships
   - Queries from TuGraph and returns JSON results

6. **Version Information Endpoint** (`/api/crates/{...}/{...}/{...}/{...}/versions`)
   - Gets all historical versions of the current crate
   - Retrieves relevant information for each version from PG database
   - Includes publication time, download count, dependent information, etc.

**Frontend and Backend Feature Addition Process**:
1. Frontend designs interface and routes, request types, parameters, return value types
2. Backend adds routes in lib.rs
3. Implements API functionality
4. Performs frontend and backend integration testing

### 2.3 model Module

**Functionality**: Defines data models used in the system

**Main Components**:
- `general_model`: Model used during crate analysis, unrelated to TuGraph
- `repo_sync_model`: Kafka message data format, each message corresponds to a Model instance
- `tugraph_model`: TuGraph data model, repo_import writes it in JSON format

The data structures defined in this module are called by multiple modules, including:
- repo_import module uses repo_sync_model when reading Kafka messages
- data_transporter module uses tugraph_model to handle TuGraph-related data

### 2.4 repo_import Module

**Functionality**: System foundation module, responsible for data acquisition and parsing

**Core Components**:
- `lib.rs`: Module entry point, encapsulates parsing function `import_from_mq_for_a_message`
- `crate_info.rs`: Logic for parsing Cargo.toml files
- `git.rs`: Git operation related functions
- `utils.rs`: Helper functions
- `version_info.rs`: Handles individual crate version information and dependency relationships

**Workflow**:
1. Sequentially reads messages from the Kafka message queue
2. Parses messages to obtain crate-related information and URL location
3. Clones the corresponding crate files from the mega database to local
4. Splits the git repository into multiple version folders by tag
5. Parses the Cargo.toml file for each version
6. Extracts and stores crate information and dependency relationships

Currently, there are approximately 170,000 full messages in Kafka, with incremental data continuing to be added in the future.

### 2.5 search Module

**Functionality**: Implements crate search functionality

**Working Principle**:
- Current implementation is based on crate name search and sorted by relevance
- Called through the `/api/search` interface
- Large models can be introduced in the future to implement more ranking strategies

### 2.6 sync_tool Module

**Functionality**: Handles incremental logic for Kafka message queue

**Characteristics**:
- Responsible for generating Kafka messages
- Independent from the main crates-pro system
- Requires separate deployment

## 3. Backend Development Guide

### 3.1 Development Process Example

Taking the search module as an example:

1. **Understand Project Structure**
   - Master the Rust project management mechanism
   - Check the root directory Cargo.toml file to understand project structure

2. **Module Design**
   - Each folder in the root directory can be seen as an independent lib-type crate (except crates_pro)
   - Independent functionality can be designed as a separate crate

3. **Functionality Implementation**
   - Implement core functionality according to requirements
   - Consider interfaces and interactions with other modules

4. **Integration into the Project**
   - Integrate new functionality into the entire project using the Rust project management mechanism

### 3.2 Code Standards

- Follow Rust standard coding style
- Maintain module independence, communicate through clear interfaces
- Write detailed comments, especially for key algorithms and complex logic
- Consider extensibility and maintainability when developing new features

### 3.3 Data Flow

```
Kafka Message Queue → repo_import parsing → TuGraph storage → data_transporter reading → Frontend display
                                          ↘ PostgreSQL storage → search module query ↗
```

This architecture ensures efficient data processing and querying while supporting rich feature extensions.