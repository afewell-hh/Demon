// Library interface for operate-ui

pub mod jetstream;
pub mod routes;

use anyhow::Result;
use axum::{
    handler::HandlerWithoutStateExt,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, get_service, post},
    Router,
};
use tera::Tera;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct AppState {
    pub jetstream_client: Option<jetstream::JetStreamClient>,
    pub tera: Tera,
    pub admin_token: Option<String>,
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

        // Load templates using crate-absolute path for deterministic resolution
        let tpl_glob = format!("{}/templates/**/*.html", env!("CARGO_MANIFEST_DIR"));
        let mut tera = match Tera::new(&tpl_glob) {
            Ok(t) => t,
            Err(e) => {
                error!("Parsing error for Tera templates ({}): {}", tpl_glob, e);
                std::process::exit(1);
            }
        };

        // Register JSON filters for template rendering (`json` and `tojson` aliases)
        let tojson = |value: &tera::Value,
                      _: &std::collections::HashMap<String, tera::Value>|
         -> tera::Result<tera::Value> {
            match serde_json::to_string_pretty(value) {
                Ok(json_string) => Ok(tera::Value::String(json_string)),
                Err(e) => Err(tera::Error::msg(format!(
                    "Failed to serialize to JSON: {}",
                    e
                ))),
            }
        };
        tera.register_filter("json", tojson);
        tera.register_filter("tojson", tojson);

        let admin_token = std::env::var("ADMIN_TOKEN").ok();

        Self {
            jetstream_client,
            tera,
            admin_token,
        }
    }
}

// Custom error type for better error handling
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

impl From<tera::Error> for AppError {
    fn from(err: tera::Error) -> Self {
        AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Template rendering error: {}", err),
        }
    }
}

impl From<Box<tera::Error>> for AppError {
    fn from(err: Box<tera::Error>) -> Self {
        AppError {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Template rendering error: {}", err),
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

// Health check endpoint
async fn health() -> impl IntoResponse {
    "OK"
}

// Fallback handler for 404s
async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Html(
            r#"
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
    "#,
        ),
    )
}

async fn handle_static_file_error() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Static file not found").into_response()
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        // Legacy routes (redirect to default tenant)
        .route("/runs", get(routes::list_runs_html))
        .route("/runs/:run_id", get(routes::get_run_html))
        // Tenant-aware HTML routes
        .route("/tenants/:tenant/runs", get(routes::list_runs_html_tenant))
        .route(
            "/tenants/:tenant/runs/:run_id",
            get(routes::get_run_html_tenant),
        )
        .route("/api/runs", get(routes::list_runs_api))
        .route("/api/runs/:run_id", get(routes::get_run_api))
        .route(
            "/api/runs/:run_id/events/stream",
            get(routes::stream_run_events_sse),
        )
        // Tenant-aware routes
        .route(
            "/api/tenants/:tenant/runs",
            get(routes::list_runs_api_tenant),
        )
        .route(
            "/api/tenants/:tenant/runs/:run_id",
            get(routes::get_run_api_tenant),
        )
        .route(
            "/api/tenants/:tenant/runs/:run_id/events/stream",
            get(routes::stream_run_events_sse_tenant),
        )
        .route(
            "/admin/templates/report",
            get(routes::admin_templates_report),
        )
        // Approvals endpoints (publish Granted/Denied)
        .route(
            "/api/approvals/:run_id/:gate_id/grant",
            post(routes::grant_approval_api),
        )
        .route(
            "/api/approvals/:run_id/:gate_id/deny",
            post(routes::deny_approval_api),
        )
        .route(
            "/api/tenants/:tenant/approvals/:run_id/:gate_id/grant",
            post(routes::grant_approval_api_tenant),
        )
        .route(
            "/api/tenants/:tenant/approvals/:run_id/:gate_id/deny",
            post(routes::deny_approval_api_tenant),
        )
        .route(
            "/static/*path",
            get_service(
                ServeDir::new("static").not_found_service(handle_static_file_error.into_service()),
            ),
        )
        .fallback(not_found)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .with_state(state)
}
