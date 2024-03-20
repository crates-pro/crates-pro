// 引入 Crate 的主要库以便能够访问 Server 结构和相关配置
extern crate database;
use database::quary_server;
use std::{sync::Arc, time::Duration};
use tokio::sync::oneshot;

#[tokio::test]
async fn test_server_responses() {
    // 使用 oneshot 信道来确保服务器可以在测试结束时停止
    let (tx, rx) = oneshot::channel::<()>();

    // 启动服务器的代码可能需要进行调整以支持优雅关闭
    tokio::spawn(async move {
        let server = Arc::new(quary_server::Server::new());
        server
            .start()
            .await
            .unwrap_or_else(|e| eprintln!("Server error: {}", e));
    });

    // 短暂延时以确保服务器启动
    tokio::time::sleep(Duration::from_secs(1)).await;

    let client = reqwest::Client::new();

    // 替换以下 URL 和路径为你的实际 API 路径
    let resp = client
        .get("http://localhost:3000/crates/test_crate")
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    println!("{}", resp.text().await.unwrap());

    // 向服务器发送停止信号
    let _ = tx.send(());

    // 等待服务器优雅关闭，这可能需要你的服务器支持某种形式的停止信号处理
    // 这里我们省略了具体实现细节

    // 更进一步的断言可以放在这里
}
