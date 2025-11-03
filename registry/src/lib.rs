//! Schema Registry Service
//!
//! Provides REST API and NATS JetStream KV integration for contract schema management.

pub mod auth;
pub mod kv;
pub mod routes;

use anyhow::Result;
use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;
use tracing::info;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub kv_client: kv::KvClient,
}

impl AppState {
    /// Create new application state with JetStream KV client
    pub async fn new() -> Result<Self> {
        let kv_client = kv::KvClient::new().await?;
        info!("Successfully initialized Schema Registry application state");
        Ok(Self { kv_client })
    }
}

/// Custom error type for API responses
#[derive(Debug)]
pub struct AppError {
    pub status_code: StatusCode,
    pub message: String,
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Internal server error: {}", err),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status_code, self.message).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Health check endpoint
async fn healthz() -> impl IntoResponse {
    "OK"
}

/// Create the Axum application router
pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(
            "/registry/contracts",
            get(routes::list_contracts).post(routes::publish_contract),
        )
        .route(
            "/registry/contracts/:name/:version",
            get(routes::get_contract),
        )
        .layer(middleware::from_fn(auth::jwt_middleware))
        // Avoid logging request headers so Authorization tokens never reach logs.
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
