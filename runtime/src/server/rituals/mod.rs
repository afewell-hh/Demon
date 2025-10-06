mod models;
mod registry;
mod runner;
mod service;
mod store;

pub use models::*;
pub use registry::AppPackRegistry;
pub use runner::{EngineRitualRunner, ExecutionPlan, RitualRunner};
pub use service::RitualService;
pub use store::RunStore;

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use std::sync::Arc;

/// Build the ritual API router.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/:ritual/runs",
            post(schedule_ritual_run).get(list_ritual_runs),
        )
        .route("/:ritual/runs/:run_id", get(get_ritual_run))
        .route(
            "/:ritual/runs/:run_id/envelope",
            get(get_ritual_run_envelope),
        )
}

#[derive(Debug, Deserialize)]
struct ListRunsQuery {
    pub app: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunLookupQuery {
    pub app: String,
}

async fn schedule_ritual_run(
    Extension(service): Extension<Arc<RitualService>>,
    Path(ritual): Path<String>,
    Json(request): Json<RitualInvocationRequest>,
) -> Response {
    if request.app.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Field 'app' must be provided"})),
        )
            .into_response();
    }

    match service.schedule_run(&ritual, request).await {
        Ok((_record, response)) => (StatusCode::ACCEPTED, Json(response)).into_response(),
        Err(err) => {
            let message = err.to_string();
            let status = classify_error(&message);
            warn!(%ritual, error = %message, "failed to schedule ritual run");
            (status, Json(json!({ "error": message }))).into_response()
        }
    }
}

async fn list_ritual_runs(
    Extension(service): Extension<Arc<RitualService>>,
    Path(ritual): Path<String>,
    Query(query): Query<ListRunsQuery>,
) -> Response {
    if query.app.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Query parameter 'app' is required"})),
        )
            .into_response();
    }

    let status_filter = match query.status.as_deref() {
        Some(raw) => match RunStatus::parse(raw) {
            Some(status) => Some(status),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Invalid status filter",
                        "allowed": ["Pending", "Running", "Completed", "Failed"],
                    })),
                )
                    .into_response();
            }
        },
        None => None,
    };

    match service
        .list_runs(&query.app, &ritual, query.limit, status_filter)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => {
            let message = err.to_string();
            let status = classify_error(&message);
            warn!(%ritual, error = %message, "failed to list ritual runs");
            (status, Json(json!({ "error": message }))).into_response()
        }
    }
}

async fn get_ritual_run(
    Extension(service): Extension<Arc<RitualService>>,
    Path((ritual, run_id)): Path<(String, String)>,
    Query(query): Query<RunLookupQuery>,
) -> Response {
    if query.app.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Query parameter 'app' is required"})),
        )
            .into_response();
    }

    match service.get_run(&query.app, &ritual, &run_id).await {
        Ok(Some(detail)) => (StatusCode::OK, Json(detail)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Run not found",
                "app": query.app,
                "ritual": ritual,
                "runId": run_id
            })),
        )
            .into_response(),
        Err(err) => {
            let message = err.to_string();
            let status = classify_error(&message);
            warn!(run = %run_id, error = %message, "failed to fetch run detail");
            (status, Json(json!({ "error": message }))).into_response()
        }
    }
}

async fn get_ritual_run_envelope(
    Extension(service): Extension<Arc<RitualService>>,
    Path((ritual, run_id)): Path<(String, String)>,
    Query(query): Query<RunLookupQuery>,
) -> Response {
    if query.app.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Query parameter 'app' is required"})),
        )
            .into_response();
    }

    match service.get_envelope(&query.app, &ritual, &run_id).await {
        Ok(Some(envelope)) => (
            StatusCode::OK,
            Json(json!({ "runId": run_id, "envelope": envelope })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "Result envelope not available",
                "app": query.app,
                "ritual": ritual,
                "runId": run_id
            })),
        )
            .into_response(),
        Err(err) => {
            let message = err.to_string();
            let status = classify_error(&message);
            warn!(run = %run_id, error = %message, "failed to fetch run envelope");
            (status, Json(json!({ "error": message }))).into_response()
        }
    }
}

fn classify_error(message: &str) -> StatusCode {
    if message.contains("not installed")
        || message.contains("not defined")
        || message.contains("not found")
    {
        StatusCode::NOT_FOUND
    } else if message.contains("must") || message.contains("cannot") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
