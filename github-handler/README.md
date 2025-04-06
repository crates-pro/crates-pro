# GitHub仓库贡献者分析工具

这是一个用Rust编写的命令行工具，用于分析GitHub仓库的贡献者信息，包括他们的基本信息、地理位置分布和贡献统计。该工具特别关注识别来自中国的贡献者，通过分析Git提交记录中的时区信息来实现。所有数据都会被存储到PostgreSQL数据库中进行持久化和高级查询。

## 功能特点

- **仓库管理**：根据GitHub仓库URL注册并分析仓库
- **贡献者信息收集**：获取仓库的所有贡献者信息（包括登录名、邮箱、位置等）
- **地理位置分析**：
  - 分析贡献者的地理位置（特别是识别来自中国的贡献者）
  - 基于Git提交历史中的时区信息进行分析
  - 生成中国贡献者比例报告
- **统计功能**：
  - 生成贡献者统计报告
  - 查询仓库的顶级贡献者列表
  - 计算中国贡献者占比
- **API优化**：
  - 支持GitHub API令牌轮换，避免触发API速率限制
  - 智能处理API请求，包括错误重试和速率限制处理
- **数据存储**：将所有数据存储到PostgreSQL数据库，支持高级查询和统计

## 技术实现

- **异步处理**：使用Tokio异步运行时处理并发请求
- **API调用**：使用Reqwest库与GitHub API交互
- **数据库交互**：使用Sea-ORM进行数据库操作，提供类型安全的查询
- **Git操作**：使用本地Git命令分析仓库提交历史
- **时区分析**：通过解析Git提交记录中的时区信息判断贡献者可能的地理位置
- **令牌轮换**：实现令牌池和轮换机制，最大化API使用效率
- **命令行界面**：使用Clap库构建友好的命令行界面

## 安装

### 前提条件

- Rust编译环境（rustc, cargo）
- PostgreSQL数据库
- Git

### 安装步骤

1. 克隆仓库：
   ```bash
   git clone <仓库URL>
   cd github-handler
   ```

2. 编译项目：
   ```bash
   cargo build --release
   ```

3. 创建配置文件：
   ```bash
   cp config.sample.json config.json
   ```

4. 编辑配置文件，添加GitHub令牌和数据库连接信息。

## 配置

可以通过以下两种方式进行配置：

### 1. 配置文件（推荐）

创建一个`config.json`文件，格式如下：

```json
{
  "github": {
    "tokens": [
      "YOUR_GITHUB_TOKEN_1",
      "YOUR_GITHUB_TOKEN_2",
      "YOUR_GITHUB_TOKEN_3"
    ]
  },
  "database": {
    "url": "postgresql://username:password@localhost:5432/dbname"
  }
}
```

### 2. 环境变量

也可以使用环境变量进行配置：

- `GITHUB_TOKEN`: 单个GitHub令牌
- `GITHUB_TOKEN_1`, `GITHUB_TOKEN_2`, ... : 多个GitHub令牌（用于轮换）
- `DATABASE_URL`: PostgreSQL数据库连接URL
- `CONFIG_PATH`: 可选，指定配置文件的路径

## 使用方法

### 注册仓库

将GitHub仓库注册到系统中，支持两种格式的URL：

```bash
cargo run -- register --url https://github.com/owner/repo
# 或
cargo run -- register --url owner/repo
```

或者使用编译后的二进制文件：

```bash
./github-handler register --url https://github.com/owner/repo
```

### 分析仓库贡献者

分析指定仓库的所有贡献者，包括基本信息和地理位置分析：

```bash
cargo run -- analyze owner repo
```

这将执行以下操作：
1. 获取仓库所有贡献者列表
2. 收集每个贡献者的详细信息
3. 克隆仓库到本地（如果尚未克隆）
4. 分析Git提交历史中的时区信息
5. 判断贡献者可能的地理位置（特别是识别中国贡献者）
6. 将所有信息存储到数据库

### 查询仓库贡献者统计

查询指定仓库的贡献者统计信息，包括中国贡献者比例：

```bash
cargo run -- query owner repo
```

### 生成贡献者地理位置分析报告

直接对本地Git仓库进行贡献者地理位置分析，生成报告：

```bash
cargo run -- --analyze-contributors /path/to/local/repo/clone
```

### 生成示例配置文件

生成一个示例配置文件：

```bash
cargo run -- --sample-config config.json
```

## 数据库架构

该工具使用PostgreSQL数据库存储以下信息：

- **github_users**: GitHub用户信息（ID、登录名、名称、邮箱、位置等）
- **programs**: 仓库信息（ID、名称、GitHub URL等）
- **repository_contributors**: 贡献者与仓库的关系（用户ID、仓库ID、贡献数等）
- **contributor_locations**: 贡献者地理位置信息（是否来自中国、常用时区等）

数据库模式会在首次运行时自动创建。

## 开发说明

该项目使用以下主要依赖：

- `tokio`: 异步运行时
- `reqwest`: HTTP客户端，用于GitHub API调用
- `sea-orm`: 数据库ORM
- `serde`: 序列化/反序列化
- `clap`: 命令行参数解析
- `tracing`: 日志记录
- `chrono`: 日期和时间处理
- `regex`: 正则表达式支持

### 项目结构

- `src/main.rs`: 程序入口点和CLI接口
- `src/config.rs`: 配置管理（配置文件和环境变量处理）
- `src/contributor_analysis.rs`: 贡献者地理位置分析逻辑
- `src/services/`: 服务层实现
  - `github_api.rs`: GitHub API客户端
  - `database.rs`: 数据库操作
- `src/entities/`: 数据库实体定义
  - `github_user.rs`: GitHub用户实体
  - `program.rs`: 仓库实体
  - `repository_contributor.rs`: 仓库贡献者关系实体
  - `contributor_location.rs`: 贡献者地理位置信息实体
- `src/migrations/`: 数据库迁移脚本

## 贡献

欢迎提交问题报告和改进建议。如果您想要贡献代码，请先创建一个Issue讨论您的想法。

## 许可证

[项目许可证]
