use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use tracing::{error, info, warn};

// 配置结构
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub github: GithubConfig,
    pub database: Option<DatabaseConfig>,
}

// GitHub配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GithubConfig {
    pub tokens: Vec<String>,
}

// 数据库配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

// 默认配置文件路径
const DEFAULT_CONFIG_PATH: &str = "config.json";

// 当前令牌索引，用于轮换令牌
static TOKEN_INDEX: AtomicUsize = AtomicUsize::new(0);

// 全局配置实例
static CONFIG: Lazy<Mutex<Option<Config>>> = Lazy::new(|| Mutex::new(None));

/// 加载配置文件
pub fn load_config() -> Option<Config> {
    // 首先检查环境变量中是否有配置文件路径
    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());

    if !Path::new(&config_path).exists() {
        warn!("配置文件 {} 不存在，将使用环境变量或默认值", config_path);

        // 创建默认配置
        let mut tokens = Vec::new();

        // 从环境变量获取GitHub令牌
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            if !token.is_empty() {
                tokens.push(token);
                info!("从环境变量GITHUB_TOKEN加载了1个令牌");
            }
        }

        // 尝试加载GITHUB_TOKEN_1, GITHUB_TOKEN_2等环境变量
        for i in 1..10 {
            let var_name = format!("GITHUB_TOKEN_{}", i);
            if let Ok(token) = env::var(&var_name) {
                if !token.is_empty() {
                    tokens.push(token);
                    info!("从环境变量{}加载了令牌", var_name);
                }
            }
        }

        if tokens.is_empty() {
            warn!("未找到任何GitHub令牌，API请求可能会受到限制");
        } else {
            info!("共加载了{}个GitHub令牌", tokens.len());
        }

        let database_url = env::var("DATABASE_URL").ok().filter(|s| !s.is_empty());

        let config = Config {
            github: GithubConfig { tokens },
            database: database_url.map(|url| DatabaseConfig { url }),
        };

        // 保存到全局配置实例
        *CONFIG.lock().unwrap() = Some(config.clone());

        return Some(config);
    }

    // 尝试读取和解析配置文件
    match fs::read_to_string(&config_path) {
        Ok(contents) => match serde_json::from_str::<Config>(&contents) {
            Ok(mut config) => {
                info!("从 {} 加载了配置文件", config_path);

                // 检查是否有令牌
                if config.github.tokens.is_empty() {
                    warn!("配置文件中没有GitHub令牌，尝试从环境变量加载");

                    // 从环境变量获取GitHub令牌
                    if let Ok(token) = env::var("GITHUB_TOKEN") {
                        if !token.is_empty() {
                            config.github.tokens.push(token);
                            info!("从环境变量GITHUB_TOKEN加载了令牌");
                        }
                    }
                }

                info!("共加载了{}个GitHub令牌", config.github.tokens.len());

                // 保存到全局配置实例
                *CONFIG.lock().unwrap() = Some(config.clone());

                Some(config)
            }
            Err(e) => {
                error!("解析配置文件失败: {}", e);
                None
            }
        },
        Err(e) => {
            error!("读取配置文件失败: {}", e);
            None
        }
    }
}

/// 获取GitHub令牌，支持令牌轮换
pub fn get_github_token() -> String {
    // 尝试获取配置
    let config = {
        let config_guard = CONFIG.lock().unwrap();
        if config_guard.is_none() {
            // 如果配置不存在，尝试加载
            drop(config_guard);
            load_config();
            CONFIG.lock().unwrap().clone()
        } else {
            config_guard.clone()
        }
    };

    // 从配置中获取令牌
    if let Some(config) = config {
        let tokens = &config.github.tokens;
        if tokens.is_empty() {
            warn!("没有可用的GitHub令牌");
            return String::new();
        }

        // 获取当前索引并递增
        let current_index = TOKEN_INDEX.fetch_add(1, Ordering::SeqCst) % tokens.len();

        // 返回当前索引对应的令牌
        tokens[current_index].clone()
    } else {
        warn!("配置加载失败，无法获取GitHub令牌");
        String::new()
    }
}

/// 获取数据库连接URL
pub fn get_database_url() -> String {
    // 尝试获取配置
    let config = {
        let config_guard = CONFIG.lock().unwrap();
        if config_guard.is_none() {
            // 如果配置不存在，尝试加载
            drop(config_guard);
            load_config();
            CONFIG.lock().unwrap().clone()
        } else {
            config_guard.clone()
        }
    };

    // 从配置中获取数据库URL
    if let Some(config) = config {
        if let Some(db_config) = config.database {
            return db_config.url;
        }
    }

    // 回退到环境变量
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://mega:mega@localhost:30432/cratespro".to_string())
}
