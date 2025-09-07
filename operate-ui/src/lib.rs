// Library interface for operate-ui

pub mod jetstream;
pub mod routes;

use anyhow::Result;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub jetstream_client: Option<jetstream::JetStreamClient>,
}

impl AppState {
    pub async fn new() -> Self {
        let jetstream_client = match jetstream::JetStreamClient::new().await {
            Ok(client) => {
                info!("Successfully connected to NATS JetStream");
                Some(client)
            }
            Err(e) => {
                warn!("Failed to connect to NATS JetStream: {}", e);
                None
            }
        };

        Self { jetstream_client }
    }
}

// Custom error type for better error handling
#[derive(Debug)]
pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_msg = format!("Internal server error: {}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, error_msg).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;

// Health check endpoint
async fn health() -> impl IntoResponse {
    "OK"
}

// Fallback handler for 404s
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html(r#"
<!DOCTYPE html>
<html>
<head>
    <title>404 - Not Found</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .error { color: #d32f2f; }
    </style>
</head>
<body>
    <h1 class="error">404 - Page Not Found</h1>
    <p><a href="/runs">← Back to Runs</a></p>
</body>
</html>
    "#))
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/runs", get(routes::list_runs_html))
        .route("/runs/:run_id", get(routes::get_run_html))
        .route("/api/runs", get(routes::list_runs_api))
        .route("/api/runs/:run_id", get(routes::get_run_api))
        .nest_service("/static", ServeDir::new("static"))
        .fallback(not_found)
        .layer(
            ServiceBuilder::new().layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::default().include_headers(true)),
            ),
        )
        .with_state(state)
}