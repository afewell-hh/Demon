// Library interface for operate-ui

pub mod app_packs;
pub mod contracts;
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
    pub bundle_loader: runtime::bundle::BundleLoader,
    pub app_pack_registry: Option<app_packs::AppPackRegistry>,
}

impl AppState {
    pub async fn new() -> Self {
        // Initialize JetStream client and ensure stream exists
        let jetstream_client = match Self::init_jetstream().await {
            Ok(client) => {
                info!("Successfully connected to NATS JetStream");
                Some(client)
            }
            Err(e) => {
                warn!("Failed to connect to NATS JetStream: {}", e);
                None
            }
        };

        // Load templates with fallback handling
        let tpl_glob = format!("{}/templates/**/*.html", env!("CARGO_MANIFEST_DIR"));
        let mut tera = match Tera::new(&tpl_glob) {
            Ok(t) => {
                // Check if any templates were actually loaded
                let template_names: Vec<&str> = t.get_template_names().collect();
                if template_names.is_empty() {
                    warn!("No templates found at {}, creating fallback", tpl_glob);
                    Self::create_fallback_tera()
                } else {
                    info!(
                        "Successfully loaded {} Tera templates from: {}",
                        template_names.len(),
                        tpl_glob
                    );
                    t
                }
            }
            Err(e) => {
                error!("Failed to load Tera templates from {}: {}", tpl_glob, e);
                warn!("Creating fallback Tera instance with minimal templates");
                Self::create_fallback_tera()
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

        // Register json_query filter for nested field access (e.g., "counts.validated")
        let json_query = |value: &tera::Value,
                          args: &std::collections::HashMap<String, tera::Value>|
         -> tera::Result<tera::Value> {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| tera::Error::msg("json_query requires 'query' parameter"))?;

            let parts: Vec<&str> = query.split('.').collect();
            let mut current = value;

            for part in parts {
                current = current.get(part).ok_or_else(|| {
                    tera::Error::msg(format!("Field '{}' not found in path '{}'", part, query))
                })?;
            }

            Ok(current.clone())
        };
        tera.register_filter("json_query", json_query);

        let admin_token = std::env::var("ADMIN_TOKEN").ok();

        // Initialize bundle loader
        let bundle_loader = runtime::bundle::BundleLoader::new(None);

        // Initialize App Pack registry
        let app_pack_registry = match app_packs::AppPackRegistry::load() {
            Ok(registry) => {
                info!("Successfully loaded App Pack registry");
                Some(registry)
            }
            Err(e) => {
                warn!("Failed to load App Pack registry: {}", e);
                None
            }
        };

        Self {
            jetstream_client,
            tera,
            admin_token,
            bundle_loader,
            app_pack_registry,
        }
    }

    /// Initialize JetStream client and ensure required stream exists
    async fn init_jetstream() -> Result<jetstream::JetStreamClient> {
        let client = jetstream::JetStreamClient::new().await?;

        // Ensure the stream exists for ritual events
        Self::ensure_stream_exists().await?;

        Ok(client)
    }

    /// Ensure the required JetStream stream exists
    async fn ensure_stream_exists() -> Result<()> {
        let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
        let nats_client = async_nats::connect(&url).await?;
        let jetstream = async_nats::jetstream::new(nats_client);

        // Resolve stream name with precedence: RITUAL_STREAM_NAME -> RITUAL_EVENTS (default)
        let stream_name =
            std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".to_string());

        match jetstream.get_stream(&stream_name).await {
            Ok(_) => {
                info!("JetStream stream '{}' already exists", stream_name);
                Ok(())
            }
            Err(_) => {
                info!("Creating JetStream stream '{}'", stream_name);
                let stream_config = async_nats::jetstream::stream::Config {
                    name: stream_name.clone(),
                    subjects: vec!["demon.ritual.v1.>".to_string()],
                    ..Default::default()
                };
                jetstream.get_or_create_stream(stream_config).await?;
                info!("Successfully created JetStream stream '{}'", stream_name);
                Ok(())
            }
        }
    }

    /// Create a minimal fallback Tera instance with basic error templates
    fn create_fallback_tera() -> Tera {
        let mut tera = Tera::default();

        // Add a basic error template
        let error_template = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Error - Demon Operate UI</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .error { color: #d32f2f; }
        .container { max-width: 600px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="container">
        <h1 class="error">Service Temporarily Unavailable</h1>
        <p>The Operate UI is experiencing template loading issues. Please contact the system administrator.</p>
        <p><a href="/health">Check System Health</a></p>
    </div>
</body>
</html>
        "#;

        // Add basic runs list fallback template
        let runs_fallback_template = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Runs - Demon Operate UI</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .error { color: #d32f2f; }
        .warning { color: #ff9800; }
        .container { max-width: 800px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Demon Operate UI - Runs</h1>
        {% if error %}
            <div class="error">
                <strong>Error:</strong> {{ error }}
            </div>
        {% endif %}
        {% if not jetstream_available %}
            <div class="warning">
                <strong>Warning:</strong> JetStream is not available. Unable to retrieve runs.
            </div>
        {% endif %}
        <p><a href="/health">System Health</a></p>
    </div>
</body>
</html>
        "#;

        if let Err(e) = tera.add_raw_template("error.html", error_template) {
            error!("Failed to add fallback error template: {}", e);
        }
        if let Err(e) = tera.add_raw_template("runs_list.html", runs_fallback_template) {
            error!("Failed to add fallback runs template: {}", e);
        }

        tera
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
        // Contract validation endpoints
        .route(
            "/api/contracts/validate/envelope",
            post(contracts::validate_envelope_endpoint),
        )
        .route(
            "/api/contracts/validate/envelope/bulk",
            post(contracts::validate_envelope_bulk_endpoint),
        )
        // Contract bundle status endpoint
        .route(
            "/api/contracts/status",
            get(contracts::bundle_status_endpoint),
        )
        // Graph viewer
        .route("/graph", get(routes::graph_viewer_html))
        // App Pack cards viewer
        .route("/app-pack-cards", get(routes::app_pack_cards_html))
        .route("/api/app-pack-cards", get(routes::app_pack_cards_api))
        // Schema form renderer
        .route("/ui/form", get(routes::schema_form_html))
        .route("/api/schema/metadata", get(routes::schema_metadata_api))
        .route("/api/form/submit", post(routes::submit_form_api))
        // Workflow viewer
        .route("/ui/workflow", get(routes::workflow_viewer_html))
        .route("/api/workflows", get(routes::list_workflows_api))
        .route("/api/workflow/metadata", get(routes::workflow_metadata_api))
        .route("/api/workflow/state", get(routes::workflow_state_api))
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
            "/api/tenants/:tenant/approvals/:run_id/:gate_id/override",
            post(routes::override_approval_api_tenant),
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
