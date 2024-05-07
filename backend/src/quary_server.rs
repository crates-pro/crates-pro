use axum::{extract::Path, routing::get, Router};
use model::crate_info::{Application, Library, Program, Version};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Default)]
pub struct Server {
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            shutdown_tx: Mutex::new(None),
        }
    }

    /// start the server at localhost:3000
    pub async fn start(self: &Arc<Self>) -> Result<(), Box<dyn Error>> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let mut tx_lock = self.shutdown_tx.lock().await; // acquire lock
        *tx_lock = Some(shutdown_tx);
        drop(tx_lock); // drop lock explicitly

        // TODO: add router
        let router = Router::new()
            .route("/crates/:name", get(Self::get_crate_info))
            .route("/crates/:name/versions", get(Self::get_crate_versions))
            .route("/libs/:name", get(Self::get_lib_info))
            .route("/apps/:name", get(Self::get_app_info));

        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let tcp = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(tcp, router)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await?;

        println!("Server running at http://{}", addr);

        Ok(())
    }

    /// close the server
    pub async fn close(self: &Arc<Self>) -> Result<(), Box<dyn Error>> {
        let maybe_tx = {
            let mut lock = self.shutdown_tx.lock().await;
            lock.take()
        };

        if let Some(tx) = maybe_tx {
            tx.send(())
                .map_err(|_| "Failed to send shutdown signal".into())
        } else {
            Err("Shutdown signal already sent or server not started.".into())
        }
    }

    async fn get_crate_info(Path(_crate_name): Path<String>) -> axum::Json<Program> {
        // TODO: fill my logic

        axum::Json(Program::default())
    }

    async fn get_crate_versions(Path(_crate_name): Path<String>) -> axum::Json<Vec<Version>> {
        // TODO: fill my logic

        axum::Json(vec![Version::default()])
    }

    async fn get_lib_info(Path(_crate_name): Path<String>) -> axum::Json<Library> {
        // TODO: fill my logic

        axum::Json(Library::default())
    }
    async fn get_app_info(Path(_crate_name): Path<String>) -> axum::Json<Application> {
        // TODO: fill my logic

        axum::Json(Application::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn app() -> Router {
        Router::new()
            .route("/crates/:name", get(Server::get_crate_info))
            .route("/crates/:name/versions", get(Server::get_crate_versions))
    }

    #[tokio::test]
    async fn test_get_crate_info() {
        let router = app().await;
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/crates/test_crate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_crate_versions() {
        let router = app().await;
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/crates/test_crate/versions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
