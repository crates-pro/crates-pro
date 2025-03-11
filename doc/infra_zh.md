# Crates Pro 系统架构文档

## 目录

- [1. 系统模块概览](#1-系统模块概览)
- [2. 核心模块详解](#2-核心模块详解)
  - [2.1 crates_pro 模块](#21-crates_pro-模块)
  - [2.2 data_transporter 模块](#22-data_transporter-模块)
  - [2.3 model 模块](#23-model-模块)
  - [2.4 repo_import 模块](#24-repo_import-模块)
  - [2.5 search 模块](#25-search-模块)
  - [2.6 sync_tool 模块](#26-sync_tool-模块)
- [3. 后端开发指南](#3-后端开发指南)

## 1. 系统模块概览

Crates Pro 由多个协同工作的模块组成，每个模块承担特定功能：

| 模块名称 | 主要功能 | 开发状态 |
|---------|----------|---------|
| crates_pro | 系统入口，任务调度 | 逻辑完备，短期无需添加功能 |
| data_transporter | 前后端连接桥梁，提供API | 需持续添加功能支持前端需求 |
| model | 数据结构定义 | 随系统发展可能需扩展 |
| repo_import | 数据导入与解析 | 解析逻辑完备，可能需添加分析功能 |
| search | Crate搜索功能 | 可引入大模型优化排序策略 |
| sync_tool | Kafka增量消息生成 | 独立部署模块 |

## 2. 核心模块详解

### 2.1 crates_pro 模块

**功能定位**：项目入口，负责启动和协调各异步任务

**核心组件**：
- `main.rs`：程序入口点，调用`core_controller.run()`启动系统
- `core_controller.rs`：定义三个关键异步任务

**异步任务说明**：

| 任务名称 | 环境变量控制 | 功能描述 |
|---------|------------|---------|
| import | CRATES_PRO_IMPORT | 读取Kafka消息队列，解析crate信息 |
| analysis | CRATES_PRO_ANALYSIS | 对crate进行分析处理 |
| package | CRATES_PRO_PACKAGE | 将数据打包到PG数据库，值为1时连接前端 |

当`CRATES_PRO_PACKAGE=1`时，系统会调用`run_api_server()`连接前端，启动API服务。

### 2.2 data_transporter 模块

**功能定位**：前后端连接桥梁，API接口提供者

**核心组件**：
- `lib.rs`：模块入口，定义API路由
- `handler.rs`：API实现逻辑
- `data_reader.rs`：TuGraph数据读取
- `db.rs`：PostgreSQL数据读取
- `data_packer.rs`：数据导入PostgreSQL

**主要API接口**：

| 接口路径 | 功能描述 |
|---------|---------|
| `/api/search` | Crate搜索，按名称相关度排序 |
| `/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}` | 获取特定Crate信息 |
| `/api/crates/{...}/{...}/{...}/{...}/dependencies` | 获取依赖项信息 |
| `/api/crates/{...}/{...}/{...}/{...}/dependencies/graph` | 获取依赖树图 |
| `/api/crates/{...}/{...}/{...}/{...}/dependents` | 获取被依赖项信息 |
| `/api/crates/{...}/{...}/{...}/{...}/versions` | 获取历史版本信息 |

**接口详解**：

1. **搜索接口** (`/api/search`)
   - 接受前端查询参数
   - 调用search模块的搜索和排序API
   - 从PG数据库按名称搜索crate并排序
   - 返回JSON格式结果

2. **Crate信息接口** (`/api/crates/{nsfront}/{nsbehind}/{cratename}/{version}`)
   - 参数说明：nsfront和nsbehind表示namespace前后部分，cratename是crate名字，version是版本
   - 使用参数从TuGraph查询解析的crate信息
   - 返回包括CVE信息、依赖关系等数据

3. **依赖信息接口** (`/api/crates/{...}/{...}/{...}/{...}/dependencies`)
   - 获取crate的依赖项信息
   - 包含直接依赖（Cargo.toml中声明）和间接依赖
   - 从TuGraph查询并返回JSON结果

4. **依赖图接口** (`/api/crates/{...}/{...}/{...}/{...}/dependencies/graph`)
   - 获取依赖项信息并构建依赖树
   - 返回JSON格式的依赖树
   - 前端通过可视化工具展示为依赖图

5. **被依赖信息接口** (`/api/crates/{...}/{...}/{...}/{...}/dependents`)
   - 获取哪些crate依赖于当前crate
   - 包含直接和间接被依赖关系
   - 从TuGraph查询并返回JSON结果

6. **版本信息接口** (`/api/crates/{...}/{...}/{...}/{...}/versions`)
   - 获取当前crate的所有历史版本
   - 从PG数据库获取每个版本的相关信息
   - 包括发布时间、下载量、被依赖信息等

**前后端功能添加流程**：
1. 由前端设计接口及路由、请求类型、参数、返回值类型
2. 后端在lib.rs中添加路由
3. 实现API功能
4. 进行前后端联调

### 2.3 model 模块

**功能定位**：定义系统中使用的数据模型

**主要组件**：
- `general_model`：Crate分析时的模型，与TuGraph无关
- `repo_sync_model`：Kafka消息数据格式，每条消息对应一个Model实例
- `tugraph_model`：TuGraph的数据模型，repo_import将其写为JSON格式

本模块定义的数据结构被多个模块调用，包括：
- repo_import模块读取Kafka消息时使用repo_sync_model
- data_transporter模块使用tugraph_model处理TuGraph相关数据

### 2.4 repo_import 模块

**功能定位**：系统基础模块，负责数据获取和解析

**核心组件**：
- `lib.rs`：模块入口，封装解析函数`import_from_mq_for_a_message`
- `crate_info.rs`：解析Cargo.toml文件逻辑
- `git.rs`：Git操作相关函数
- `utils.rs`：辅助函数
- `version_info.rs`：处理单个crate版本信息和依赖关系

**工作流程**：
1. 按序读取Kafka消息队列中的消息
2. 解析消息获取crate相关信息和所在URL
3. 从mega数据库克隆对应的crate文件到本地
4. 按tag将git仓库拆分成多个版本文件夹
5. 解析每个版本的Cargo.toml文件
6. 提取并存储crate信息和依赖关系

目前Kafka全量消息约17万条，未来会持续增加增量数据。

### 2.5 search 模块

**功能定位**：实现crate搜索功能

**工作原理**：
- 当前实现基于crate名称搜索并按相关度排序
- 通过`/api/search`接口被调用
- 后续可引入大模型实现更多排序策略

### 2.6 sync_tool 模块

**功能定位**：处理Kafka消息队列的增量逻辑

**特点**：
- 负责生成Kafka消息
- 独立于crates-pro主系统
- 需单独部署

## 3. 后端开发指南

### 3.1 开发流程示例

以search模块为例：

1. **了解项目结构**
   - 掌握Rust项目管理机制
   - 查看根目录Cargo.toml文件了解项目结构

2. **模块设计**
   - 根目录下每个文件夹可视为独立lib类型crate（crates_pro除外）
   - 可将独立功能设计为单独crate

3. **功能实现**
   - 根据需求实现core功能
   - 考虑与其他模块的接口和交互

4. **集成到项目**
   - 通过Rust项目管理机制将新功能集成到整个项目

### 3.2 代码规范

- 遵循Rust标准编码风格
- 保持模块独立性，通过明确的接口通信
- 编写详细注释，特别是关键算法和复杂逻辑
- 新功能开发时考虑扩展性和可维护性

### 3.3 数据流向

```
Kafka消息队列 → repo_import解析 → TuGraph存储 → data_transporter读取 → 前端展示
                                  ↘ PostgreSQL存储 → search模块查询 ↗
```

这种架构确保了数据的高效处理和查询，同时支持丰富的功能扩展。