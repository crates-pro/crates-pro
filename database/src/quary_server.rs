use crate::crate_info::{CrateInfo, CrateVersion};
use axum::{extract::Path, routing::get, Router};

use std::error::Error;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

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

        let app = Router::new()
            .route("/crates/:name", get(Self::get_crate_info))
            .route("/crates/:name/versions", get(Self::get_crate_versions));

        let addr = "127.0.0.1:3000".parse().unwrap();
        let server = axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            });

        println!("Server running at http://{}", addr);

        server.await.map_err(|e| e.into())
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

    async fn get_crate_info(Path(crate_name): Path<String>) -> axum::Json<CrateInfo> {
        // TODO: fill my logic

        axum::Json(CrateInfo::default())
    }

    async fn get_crate_versions(Path(crate_name): Path<String>) -> axum::Json<Vec<CrateVersion>> {
        // TODO: fill my logic

        axum::Json(vec![CrateVersion::default()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        response::Response,
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
