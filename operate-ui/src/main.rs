use anyhow::{Context, Result};
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::env;
use tower::ServiceBuilder;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod jetstream;
mod routes;

#[derive(Clone)]
pub struct AppState {
    jetstream_client: Option<jetstream::JetStreamClient>,
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
    Html(r#"
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
    "#)
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/runs", get(routes::list_runs_html))
        .route("/runs/:run_id", get(routes::get_run_html))
        .route("/api/runs", get(routes::list_runs_api))
        .route("/api/runs/:run_id", get(routes::get_run_api))
        .nest_service("/static", ServeDir::new("operate-ui/static"))
        .fallback(not_found)
        .layer(
            ServiceBuilder::new().layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::default().include_headers(true)),
            ),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "operate_ui=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get configuration from environment
    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .context("PORT must be a valid number")?;

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());

    // Initialize application state
    let state = AppState::new().await;

    // Create the application
    let app = create_app(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", bind_addr, port))
        .await
        .with_context(|| format!("Failed to bind to {}:{}", bind_addr, port))?;

    info!("Server starting on http://{}:{}", bind_addr, port);

    axum::serve(listener, app)
        .await
        .context("Server failed to start")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = AppState {
            jetstream_client: None,
        };
        let app = create_app(state);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_404_handling() {
        let state = AppState {
            jetstream_client: None,
        };
        let app = create_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK); // HTML response with 404 content
    }
}