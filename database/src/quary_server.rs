use crate::crate_info::{self, CrateInfo, CrateVersion};
use axum::{
    extract::Path,
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

async fn hello_world() -> &'static str {
    "Hello, world!"
}

// 定义一个用户结构体
#[derive(Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
}

async fn list_users() -> &'static str {
    "Returning list of users..."
}

async fn create_user(Json(payload): Json<User>) -> (StatusCode, &'static str) {
    println!("Creating user: {}", payload.name);
    (StatusCode::CREATED, "User created")
}

async fn update_user(Json(payload): Json<User>) -> &'static str {
    println!("Updating user: {}", payload.name);
    "User updated"
}

async fn delete_user() -> &'static str {
    "User deleted"
}

async fn get_crate_info(Path(crate_name): Path<String>) -> axum::Json<CrateInfo> {
    // TODO: fill my logic
    let info = CrateInfo::new(
        crate_name,
        "1.0.0".to_string(),
        Some("A utility crate".to_string()),
        Some("https://docs.rs/my_crate".to_string()),
        Some("https://github.com/example/my_crate".to_string()),
        Some("MIT".to_string()),
        5,
    );
    axum::Json(info)
}

async fn get_crate_versions(Path(crate_name): Path<String>) -> axum::Json<Vec<CrateVersion>> {
    // TODO: this is a demo
    let versions = vec![
        CrateVersion::new(
            "0.1.0".to_string(),
            Utc::now().date_naive(),
            Some("Initial release".to_string()),
        ),
        CrateVersion::new(
            "0.1.1".to_string(),
            Utc::now().date_naive(),
            Some("Fixed minor bugs".to_string()),
        ),
    ];

    axum::Json(versions)
}

// 提供一个公共函数来启动服务器
pub async fn start_quary_server() {
    // 创建路由
    let app = Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/:id", put(update_user).delete(delete_user))
        .route("/crates/:name", get(get_crate_info))
        .route("/crates/:name/versions", get(get_crate_versions));

    // 运行应用
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
