use crate::jetstream::{RunDetail, RunSummary};
use crate::{AppError, AppState};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use tracing::{debug, error, info};

// Query parameters for list runs API
#[derive(Deserialize)]
pub struct ListRunsQuery {
    limit: Option<usize>,
}

/// List runs - HTML response
#[axum::debug_handler]
pub async fn list_runs_html(
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Html<String> {
    debug!(
        "Handling HTML request to list runs with limit: {:?}",
        query.limit
    );

    let (runs, error) = match &state.jetstream_client {
        Some(client) => match client.list_runs(query.limit).await {
            Ok(runs) => {
                info!("Successfully retrieved {} runs for HTML", runs.len());
                (runs, None)
            }
            Err(e) => {
                error!("Failed to retrieve runs: {}", e);
                (vec![], Some(format!("Failed to retrieve runs: {}", e)))
            }
        },
        None => (vec![], Some("JetStream is not available".to_string())),
    };

    let mut context = tera::Context::new();
    context.insert("runs", &runs);
    context.insert("error", &error);
    context.insert("jetstream_available", &state.jetstream_client.is_some());

    let html = state
        .tera
        .render("runs_list.html", &context)
        .map_err(|e| {
            error!("Template rendering failed: {}", e);
            AppError::from(e as tera::Error)
        })
        .unwrap_or_else(|e| {
            error!(
                "Failed to render error page after template rendering failure: {}",
                e
            );
            // Fallback to a simple error message if rendering the error page also fails
            format!(
                "<h1>Internal Server Error</h1><p>Failed to render page: {}</p>",
                e
            )
        });

    Html(html)
}

/// List runs - JSON API response
#[axum::debug_handler]
pub async fn list_runs_api(
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Response {
    debug!(
        "Handling JSON API request to list runs with limit: {:?}",
        query.limit
    );

    match &state.jetstream_client {
        Some(client) => match client.list_runs(query.limit).await {
            Ok(runs) => {
                info!("Successfully retrieved {} runs for API", runs.len());
                Json(runs).into_response()
            }
            Err(e) => {
                error!("Failed to retrieve runs: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to retrieve runs: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => {
            error!("JetStream client not available");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "JetStream is not available"
                })),
            )
                .into_response()
        }
    }
}

/// Get run detail - HTML response
#[axum::debug_handler]
pub async fn get_run_html(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Html<String> {
    debug!("Handling HTML request for run detail: {}", run_id);

    let (run, error) = match &state.jetstream_client {
        Some(client) => match client.get_run_detail(&run_id).await {
            Ok(run) => {
                if run.is_some() {
                    info!("Successfully retrieved run detail for HTML: {}", run_id);
                } else {
                    info!("Run not found: {}", run_id);
                }
                (run, None)
            }
            Err(e) => {
                error!("Failed to retrieve run detail: {}", e);
                (None, Some(format!("Failed to retrieve run: {}", e)))
            }
        },
        None => (None, Some("JetStream is not available".to_string())),
    };

    let mut context = tera::Context::new();
    context.insert("run", &run);
    context.insert("error", &error);
    context.insert("jetstream_available", &state.jetstream_client.is_some());
    context.insert("run_id", &run_id);

    let html = state
        .tera
        .render("run_detail.html", &context)
        .map_err(|e| {
            error!("Template rendering failed: {}", e);
            AppError::from(e as tera::Error)
        })
        .unwrap_or_else(|e| {
            error!(
                "Failed to render error page after template rendering failure: {}",
                e
            );
            // Fallback to a simple error message if rendering the error page also fails
            format!(
                "<h1>Internal Server Error</h1><p>Failed to render page: {}</p>",
                e
            )
        });

    Html(html)
}

/// Get run detail - JSON API response
#[axum::debug_handler]
pub async fn get_run_api(State(state): State<AppState>, Path(run_id): Path<String>) -> Response {
    debug!("Handling JSON API request for run detail: {}", run_id);

    match &state.jetstream_client {
        Some(client) => match client.get_run_detail(&run_id).await {
            Ok(Some(run)) => {
                info!("Successfully retrieved run detail for API: {}", run_id);
                Json(run).into_response()
            }
            Ok(None) => {
                info!("Run not found: {}", run_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": "Run not found",
                        "runId": run_id
                    })),
                )
                    .into_response()
            }
            Err(e) => {
                error!("Failed to retrieve run detail: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to retrieve run detail: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => {
            error!("JetStream client not available");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "JetStream is not available"
                })),
            )
                .into_response()
        }
    }
}

// Helper functions for templates
impl RunSummary {
    pub fn format_timestamp(&self) -> String {
        self.start_ts.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    pub fn status_class(&self) -> &'static str {
        match self.status {
            crate::jetstream::RunStatus::Running => "status-running",
            crate::jetstream::RunStatus::Completed => "status-completed",
            crate::jetstream::RunStatus::Failed => "status-failed",
        }
    }
}

impl RunDetail {
    pub fn format_timestamp(&self) -> String {
        if let Some(first_event) = self.events.first() {
            first_event.ts.format("%Y-%m-%d %H:%M:%S UTC").to_string()
        } else {
            "Unknown".to_string()
        }
    }

    pub fn status(&self) -> crate::jetstream::RunStatus {
        // Determine status from last event
        if let Some(last_event) = self.events.last() {
            match last_event.event.as_str() {
                "ritual.completed:v1" => crate::jetstream::RunStatus::Completed,
                "ritual.failed:v1" => crate::jetstream::RunStatus::Failed,
                _ => crate::jetstream::RunStatus::Running,
            }
        } else {
            crate::jetstream::RunStatus::Running
        }
    }

    pub fn status_class(&self) -> &'static str {
        match self.status() {
            crate::jetstream::RunStatus::Running => "status-running",
            crate::jetstream::RunStatus::Completed => "status-completed",
            crate::jetstream::RunStatus::Failed => "status-failed",
        }
    }
}

impl crate::jetstream::RitualEvent {
    pub fn format_timestamp(&self) -> String {
        self.ts.format("%H:%M:%S.%3f").to_string()
    }

    pub fn format_full_timestamp(&self) -> String {
        self.ts.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    pub fn event_display_name(&self) -> String {
        match self.event.as_str() {
            "ritual.started:v1" => "Ritual Started".to_string(),
            "ritual.completed:v1" => "Ritual Completed".to_string(),
            "ritual.failed:v1" => "Ritual Failed".to_string(),
            "ritual.transitioned:v1" => "State Transition".to_string(),
            "timer.scheduled:v1" => "Timer Scheduled".to_string(),
            _ => self.event.clone(),
        }
    }

    pub fn has_state_transition(&self) -> bool {
        self.state_from.is_some() || self.state_to.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jetstream::{RitualEvent, RunStatus};
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_run_summary_helpers() {
        let run = RunSummary {
            run_id: "test-run".to_string(),
            ritual_id: "test-ritual".to_string(),
            start_ts: Utc::now(),
            status: RunStatus::Completed,
        };

        assert_eq!(run.status_class(), "status-completed");
        assert!(run.format_timestamp().contains("UTC"));
    }

    #[test]
    fn test_ritual_event_helpers() {
        let event = RitualEvent {
            ts: Utc::now(),
            event: "ritual.started:v1".to_string(),
            state_from: Some("idle".to_string()),
            state_to: Some("running".to_string()),
            extra: HashMap::new(),
        };

        assert_eq!(event.event_display_name(), "Ritual Started");
        assert!(event.has_state_transition());
    }

    #[test]
    fn test_run_detail_status_determination() {
        let mut events = vec![RitualEvent {
            ts: Utc::now(),
            event: "ritual.started:v1".to_string(),
            state_from: None,
            state_to: None,
            extra: HashMap::new(),
        }];

        let run = RunDetail {
            run_id: "test".to_string(),
            ritual_id: "test".to_string(),
            events: events.clone(),
        };

        assert_eq!(run.status(), RunStatus::Running);

        events.push(RitualEvent {
            ts: Utc::now(),
            event: "ritual.completed:v1".to_string(),
            state_from: None,
            state_to: None,
            extra: HashMap::new(),
        });

        let completed_run = RunDetail {
            run_id: "test".to_string(),
            ritual_id: "test".to_string(),
            events,
        };

        assert_eq!(completed_run.status(), RunStatus::Completed);
    }
}
