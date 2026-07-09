//! HTTP API for Cortex.
//!
//! Serve with [`serve`] after constructing an [`ApiState`].

#![deny(missing_docs)]

mod dto;
mod error;
mod routes;
mod state;

pub use dto::{
    HealthResponse, InfoResponse, ModelInfo, RunRequest, RunResponse, SessionDetail, SessionInfo,
    ToolInfo, ToolResultInfo,
};
pub use error::{ApiError, ApiResult, ErrorBody};
pub use state::{ApiState, SharedState};

use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

/// Build the axum router.
pub fn router(state: ApiState) -> Router {
    let shared: SharedState = Arc::new(state);
    Router::new()
        .route("/health", get(routes::health))
        .route("/v1/info", get(routes::info))
        .route("/v1/models", get(routes::list_models))
        .route("/v1/tools", get(routes::list_tools))
        .route("/v1/sessions", get(routes::list_sessions))
        .route("/v1/sessions/:id", get(routes::get_session))
        .route("/v1/runs", post(routes::create_run))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(shared)
}

/// Bind and serve until cancelled / error.
pub async fn serve(state: ApiState, addr: SocketAddr) -> anyhow::Result<()> {
    let app = router(state);
    info!(%addr, "cortex HTTP API listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use cortex_llm::{MockProvider, ProviderRegistry};
    use cortex_memory::open_sqlite;
    use cortex_tools::{register_default_tools, ToolExecutor, ToolRegistry};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tower::ServiceExt;

    async fn test_state(token: Option<&str>) -> ApiState {
        let dir = tempdir().unwrap();
        let pool = open_sqlite(dir.path().join("t.db")).await.unwrap();
        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let mut registry = ProviderRegistry::new();
        registry.register_provider("mock", Arc::new(MockProvider::echo("hello from api test")));
        registry
            .register_alias("default", "mock", "mock-model")
            .unwrap();
        registry.set_default_alias("default");
        ApiState {
            workspace: dir.path().to_path_buf(),
            models_config: dir.path().join("models.toml"),
            database: dir.path().join("t.db"),
            registry,
            tools: ToolExecutor::new(Arc::new(reg)),
            store: cortex_memory::SessionStore::new(pool),
            default_yolo: true,
            default_max_turns: 4,
            api_token: token.map(|s| s.to_string()),
            version: "0.1.0-test".into(),
        }
    }

    #[tokio::test]
    async fn health_ok() {
        let app = router(test_state(None).await);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[tokio::test]
    async fn auth_required() {
        let state = test_state(Some("secret")).await;
        let res = router(test_state(Some("secret")).await)
            .oneshot(
                Request::builder()
                    .uri("/v1/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let res = router(state)
            .oneshot(
                Request::builder()
                    .uri("/v1/info")
                    .header("Authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn run_mock() {
        let app = router(test_state(None).await);
        let body = serde_json::json!({
            "prompt": "say hi",
            "yolo": true,
            "max_turns": 2
        });
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v["session_id"].as_str().is_some());
        assert!(v["final_message"].as_str().unwrap().contains("hello"));
    }
}
