use crate::jetstream::{RunDetail, RunSummary};
use crate::{AppError, AppState};

use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response, Sse},
    Json,
};
use chrono::DateTime;
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info};

// Query parameters for list runs API
#[derive(Deserialize, Debug, Clone)]
pub struct ListRunsQuery {
    pub status: Option<String>,
    pub capability: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub limit: Option<usize>,
}

/// Validate query parameters
fn validate_query_params(query: &ListRunsQuery) -> Result<(), String> {
    // Validate status
    if let Some(ref status) = query.status {
        let valid_statuses = ["running", "completed", "failed"];
        if !valid_statuses.contains(&status.to_lowercase().as_str()) {
            return Err(format!(
                "invalid status: {}; expected running|completed|failed",
                status
            ));
        }
    }

    // Validate since/until timestamps
    if let Some(ref since) = query.since {
        if !is_valid_timestamp(since) {
            return Err(format!(
                "invalid since timestamp: {}; expected RFC3339 or Unix seconds",
                since
            ));
        }
    }

    if let Some(ref until) = query.until {
        if !is_valid_timestamp(until) {
            return Err(format!(
                "invalid until timestamp: {}; expected RFC3339 or Unix seconds",
                until
            ));
        }
    }

    // Validate limit
    if let Some(limit) = query.limit {
        let max_limit = std::env::var("RUNS_LIST_MAX_LIMIT")
            .unwrap_or_else(|_| "200".to_string())
            .parse()
            .unwrap_or(200);
        if limit == 0 || limit > max_limit {
            return Err(format!(
                "invalid limit: {}; must be between 1 and {}",
                limit, max_limit
            ));
        }
    }

    Ok(())
}

/// Check if a timestamp string is valid (RFC3339 or Unix seconds)
fn is_valid_timestamp(ts: &str) -> bool {
    // Try parsing as RFC3339
    if DateTime::parse_from_rfc3339(ts).is_ok() {
        return true;
    }

    // Try parsing as Unix seconds (must be non-negative)
    if let Ok(seconds) = ts.parse::<i64>() {
        if seconds >= 0 && DateTime::from_timestamp(seconds, 0).is_some() {
            return true;
        }
    }

    false
}

/// List runs - HTML response
#[axum::debug_handler]
pub async fn list_runs_html(
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Html<String> {
    debug!(
        "Handling HTML request to list runs with params: {:?}",
        query
    );

    // Validate query parameters
    let validation_error = validate_query_params(&query).err();

    let (runs, error) = if let Some(err) = validation_error {
        (vec![], Some(err))
    } else {
        match &state.jetstream_client {
            Some(client) => match client.list_runs_filtered(query.clone()).await {
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
        }
    };

    let mut context = tera::Context::new();
    context.insert("runs", &runs);
    context.insert("error", &error);
    context.insert("jetstream_available", &state.jetstream_client.is_some());
    context.insert("current_page", &"runs");

    // Insert filter values for form persistence
    context.insert("filter_status", &query.status.as_deref().unwrap_or(""));
    context.insert(
        "filter_capability",
        &query.capability.as_deref().unwrap_or(""),
    );
    context.insert("filter_since", &query.since.as_deref().unwrap_or(""));
    context.insert("filter_until", &query.until.as_deref().unwrap_or(""));
    context.insert("filter_limit", &query.limit.unwrap_or(50));

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
        "Handling JSON API request to list runs with params: {:?}",
        query
    );

    // Validate query parameters
    if let Err(error) = validate_query_params(&query) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": error
            })),
        )
            .into_response();
    }

    match &state.jetstream_client {
        Some(client) => match client.list_runs_filtered(query.clone()).await {
            Ok(runs) => {
                info!("Successfully retrieved {} runs for API", runs.len());
                Json(runs).into_response()
            }
            Err(e) => {
                error!("Failed to retrieve runs: {}", e);
                let error_id = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default() as u32;
                error!("Internal error (ref: {:x}): {}", error_id, e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("internal error (ref: {:x})", error_id)
                    })),
                )
                    .into_response()
            }
        },
        None => {
            error!("JetStream client not available");
            (
                StatusCode::BAD_GATEWAY,
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
    context.insert("current_page", &"runs");

    // View helpers to avoid template method calls
    if let Some(ref rd) = run {
        // started timestamp: first event ts if present
        let started = rd.events.first().map(|e| e.ts.to_rfc3339());
        context.insert("run_started", &started);

        // status and class based on last event
        let (status, status_class) = rd
            .events
            .last()
            .map(|e| match e.event.as_str() {
                "ritual.completed:v1" => ("Completed", "status-completed"),
                "ritual.failed:v1" => ("Failed", "status-failed"),
                _ => ("Running", "status-running"),
            })
            .unwrap_or(("Running", "status-running"));
        context.insert("run_status", &status);
        context.insert("run_status_class", &status_class);

        // Approvals summary (single row): Pending/Granted/Denied with fields
        if let Some(summary) = ApprovalsSummary::from_events(&rd.events) {
            context.insert("approvals", &summary);
        }
    }

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
                    StatusCode::BAD_GATEWAY,
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
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "JetStream is not available"
                })),
            )
                .into_response()
        }
    }
}

// ---------------- Admin ----------------
#[derive(Serialize)]
pub struct TemplateReport {
    pub templates: Vec<String>,
    pub has_filter_tojson: bool,
    pub template_ready: bool,
}

/// Admin: templates/report (JSON)
pub async fn admin_templates_report(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(expected) = &state.admin_token {
        let got = headers
            .get("X-Admin-Token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if got != expected {
            return (StatusCode::UNAUTHORIZED, "missing or invalid admin token").into_response();
        }
    }

    let templates = state
        .tera
        .get_template_names()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let body = TemplateReport {
        templates,
        has_filter_tojson: true,
        template_ready: true,
    };
    Json(body).into_response()
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

    pub fn is_running(&self) -> bool {
        matches!(self.status(), crate::jetstream::RunStatus::Running)
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

// ---- Approvals summary helpers ----
#[derive(Debug, Clone, Serialize)]
struct ApprovalsSummary {
    status: String, // Pending | Granted | Denied
    #[serde(rename = "statusClass")]
    status_class: String, // badge class mapping
    #[serde(rename = "gateId")]
    gate_id: String,
    requester: Option<String>,
    approver: Option<String>,
    reason: Option<String>,
    note: Option<String>,
}

impl ApprovalsSummary {
    fn from_events(events: &[crate::jetstream::RitualEvent]) -> Option<Self> {
        // Find the latest approval.* event
        let evt = events
            .iter()
            .rev()
            .find(|e| e.event.starts_with("approval."))?;

        let gate_id = evt
            .extra
            .get("gateId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        match evt.event.as_str() {
            "approval.granted:v1" => Some(Self {
                status: "Granted".to_string(),
                status_class: "status-completed".to_string(),
                gate_id,
                requester: None,
                approver: evt
                    .extra
                    .get("approver")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                reason: None,
                note: evt
                    .extra
                    .get("note")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }),
            "approval.denied:v1" => {
                let reason = evt
                    .extra
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let status = if matches!(reason.as_deref(), Some("expired")) {
                    "Denied — expired".to_string()
                } else {
                    "Denied".to_string()
                };
                Some(Self {
                    status,
                    status_class: "status-failed".to_string(),
                    gate_id,
                    requester: None,
                    approver: evt
                        .extra
                        .get("approver")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    reason,
                    note: None,
                })
            }
            "approval.requested:v1" => Some(Self {
                status: "Pending".to_string(),
                status_class: "status-running".to_string(),
                gate_id,
                requester: evt
                    .extra
                    .get("requester")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                approver: None,
                reason: evt
                    .extra
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                note: None,
            }),
            _ => None,
        }
    }
}

// ---- Approvals grant/deny endpoints ----
#[derive(Debug, Deserialize)]
pub struct ApproveBody {
    approver: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DenyBody {
    approver: String,
    reason: String,
}

async fn publish_approval_event(
    ritual_id: &str,
    run_id: &str,
    payload: serde_json::Value,
    msg_id: String,
) -> anyhow::Result<()> {
    use async_nats::jetstream;
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client.clone());
    // Resolve stream name with precedence
    let desired = std::env::var("RITUAL_STREAM_NAME").ok();
    if let Some(name) = desired {
        let _ = js
            .get_or_create_stream(jetstream::stream::Config {
                name,
                subjects: vec!["demon.ritual.v1.>".to_string()],
                ..Default::default()
            })
            .await?;
    } else {
        // Prefer default; fall back to deprecated if it exists
        if js.get_stream("RITUAL_EVENTS").await.is_err() {
            if js.get_stream("DEMON_RITUAL_EVENTS").await.is_ok() {
                tracing::warn!("Using deprecated stream 'DEMON_RITUAL_EVENTS'; set RITUAL_STREAM_NAME or migrate to 'RITUAL_EVENTS'");
            } else {
                let _ = js
                    .get_or_create_stream(jetstream::stream::Config {
                        name: "RITUAL_EVENTS".to_string(),
                        subjects: vec!["demon.ritual.v1.>".to_string()],
                        ..Default::default()
                    })
                    .await?;
            }
        }
    }

    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Nats-Msg-Id", msg_id.as_str());
    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;
    Ok(())
}

fn approver_allowed(email: &str) -> bool {
    let allowlist = std::env::var("APPROVER_ALLOWLIST").unwrap_or_default();
    if allowlist.is_empty() {
        return false;
    }
    allowlist
        .split(',')
        .map(|s| s.trim())
        .any(|allowed| !allowed.is_empty() && allowed.eq_ignore_ascii_case(email))
}

#[axum::debug_handler]
pub async fn grant_approval_api(
    State(state): State<AppState>,
    Path((run_id, gate_id)): Path<(String, String)>,
    Json(body): Json<ApproveBody>,
) -> Response {
    if !approver_allowed(&body.approver) {
        return (StatusCode::FORBIDDEN, "approver not allowed").into_response();
    }

    // Ensure stream exists before attempting to read (if explicit name set)
    if let Ok(name) = std::env::var("RITUAL_STREAM_NAME") {
        if let Ok(client) = async_nats::connect(
            &std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string()),
        )
        .await
        {
            let js = async_nats::jetstream::new(client);
            let _ = js
                .get_or_create_stream(async_nats::jetstream::stream::Config {
                    name,
                    subjects: vec!["demon.ritual.v1.>".to_string()],
                    ..Default::default()
                })
                .await;
        }
    }

    // Discover ritualId by looking up run and enforce first-writer-wins on approvals
    let ritual_id = match &state.jetstream_client {
        Some(js) => match js.get_run_detail(&run_id).await {
            Ok(Some(rd)) => {
                // Enforce: if a terminal approval already exists for this gate, prevent conflicting writes
                if let Some(last) = rd.events.iter().rev().find(|e| {
                    (e.event == "approval.granted:v1" || e.event == "approval.denied:v1")
                        && e.extra
                            .get("gateId")
                            .and_then(|v| v.as_str())
                            .map(|g| g == gate_id)
                            .unwrap_or(false)
                }) {
                    // If already granted, duplicate grant is a no-op (200); deny is rejected (409)
                    if last.event == "approval.granted:v1" {
                        // same terminal -> no-op
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "noop",
                                "reason": "gate already granted"
                            })),
                        )
                            .into_response();
                    }
                    // Already denied => reject grant
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "gate already resolved",
                            "state": if last.event == "approval.granted:v1" { "granted" } else { "denied" }
                        })),
                    )
                        .into_response();
                }
                rd.ritual_id
            }
            Ok(None) => return (StatusCode::NOT_FOUND, "run not found").into_response(),
            Err(e) => {
                error!("get_run_detail failed: {}", e);
                return (StatusCode::BAD_GATEWAY, "JetStream error").into_response();
            }
        },
        None => return (StatusCode::BAD_GATEWAY, "JetStream unavailable").into_response(),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "event": "approval.granted:v1",
        "ts": now,
        "tenantId": "default",
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "approver": body.approver,
        "note": body.note,
    });
    let msg_id = format!(
        "{}:approval:{}:granted",
        payload["runId"].as_str().unwrap(),
        payload["gateId"].as_str().unwrap()
    );
    if let Err(e) = publish_approval_event(
        payload["ritualId"].as_str().unwrap(),
        payload["runId"].as_str().unwrap(),
        payload.clone(),
        msg_id,
    )
    .await
    {
        error!("failed to publish: {}", e);
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("publish failed: {}", e)})),
        )
            .into_response();
    }

    (StatusCode::OK, Json(payload)).into_response()
}

#[axum::debug_handler]
pub async fn deny_approval_api(
    State(state): State<AppState>,
    Path((run_id, gate_id)): Path<(String, String)>,
    Json(body): Json<DenyBody>,
) -> Response {
    if !approver_allowed(&body.approver) {
        return (StatusCode::FORBIDDEN, "approver not allowed").into_response();
    }

    // Ensure stream exists before attempting to read
    if let Some(_jsctx) = &state.jetstream_client {
        let desired = std::env::var("RITUAL_STREAM_NAME").ok();
        if let Some(name) = desired {
            let client = async_nats::connect(
                &std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string()),
            )
            .await
            .ok();
            if let Some(client) = client {
                let _ = async_nats::jetstream::new(client)
                    .get_or_create_stream(async_nats::jetstream::stream::Config {
                        name,
                        subjects: vec!["demon.ritual.v1.>".to_string()],
                        ..Default::default()
                    })
                    .await;
            }
        }
    }

    let ritual_id = match &state.jetstream_client {
        Some(js) => match js.get_run_detail(&run_id).await {
            Ok(Some(rd)) => {
                if let Some(last) = rd.events.iter().rev().find(|e| {
                    (e.event == "approval.granted:v1" || e.event == "approval.denied:v1")
                        && e.extra
                            .get("gateId")
                            .and_then(|v| v.as_str())
                            .map(|g| g == gate_id)
                            .unwrap_or(false)
                }) {
                    // Already denied -> duplicate deny is a no-op (200); grant is rejected (409)
                    if last.event == "approval.denied:v1" {
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "noop",
                                "reason": "gate already denied"
                            })),
                        )
                            .into_response();
                    }
                    // Already granted => reject deny
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "gate already resolved",
                            "state": if last.event == "approval.granted:v1" { "granted" } else { "denied" }
                        })),
                    )
                        .into_response();
                }
                rd.ritual_id
            }
            Ok(None) => return (StatusCode::NOT_FOUND, "run not found").into_response(),
            Err(e) => {
                error!("get_run_detail failed: {}", e);
                return (StatusCode::BAD_GATEWAY, "JetStream error").into_response();
            }
        },
        None => return (StatusCode::BAD_GATEWAY, "JetStream unavailable").into_response(),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "event": "approval.denied:v1",
        "ts": now,
        "tenantId": "default",
        "runId": run_id,
        "ritualId": ritual_id,
        "gateId": gate_id,
        "approver": body.approver,
        "reason": body.reason,
    });
    let msg_id = format!(
        "{}:approval:{}:denied",
        payload["runId"].as_str().unwrap(),
        payload["gateId"].as_str().unwrap()
    );
    if let Err(e) = publish_approval_event(
        payload["ritualId"].as_str().unwrap(),
        payload["runId"].as_str().unwrap(),
        payload.clone(),
        msg_id,
    )
    .await
    {
        error!("failed to publish: {}", e);
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("publish failed: {}", e)})),
        )
            .into_response();
    }

    (StatusCode::OK, Json(payload)).into_response()
}

/// Stream run events via Server-Sent Events (SSE)
#[axum::debug_handler]
pub async fn stream_run_events(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    debug!("Starting SSE stream for run: {}", run_id);

    // Configuration from environment
    let heartbeat_seconds = std::env::var("SSE_HEARTBEAT_SECONDS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);
    let replay_count = std::env::var("SSE_REPLAY_COUNT")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    // Create a stream that combines events and heartbeats
    let stream = async_stream::stream! {
        // First, replay the last N events
        if let Some(client) = &state.jetstream_client {
            match client.get_run_detail(&run_id).await {
                Ok(Some(run_detail)) => {
                    let events = &run_detail.events;
                    let start_idx = events.len().saturating_sub(replay_count);

                    for event in &events[start_idx..] {
                        let data = serde_json::to_string(&event).unwrap_or_default();
                        yield Ok(Event::default()
                            .event("message")
                            .id(format!("replay-{}", event.extra.get("messageId").and_then(|v| v.as_str()).unwrap_or("unknown")))
                            .data(data));
                    }

                    // Send a marker event to indicate replay is complete
                    yield Ok(Event::default()
                        .event("replay-complete")
                        .data("{}"));
                }
                _ => {
                    error!("Failed to get run detail for SSE stream: {}", run_id);
                }
            }

            // Now start live streaming
            // For now, we'll use a polling approach with heartbeats
            // In production, this should use NATS JetStream consumer with push
            let mut heartbeat = interval(Duration::from_secs(heartbeat_seconds));
            heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let mut last_event_count = 0;

            loop {
                heartbeat.tick().await;

                // Check for new events
                match client.get_run_detail(&run_id).await {
                    Ok(Some(run_detail)) => {
                        let events = &run_detail.events;
                        if events.len() > last_event_count {
                            // Send new events
                            for event in &events[last_event_count..] {
                                let data = serde_json::to_string(&event).unwrap_or_default();
                                yield Ok(Event::default()
                                    .event("message")
                                    .id(format!("live-{}", event.extra.get("messageId").and_then(|v| v.as_str()).unwrap_or("unknown")))
                                    .data(data));
                            }
                            last_event_count = events.len();
                        } else {
                            // Send heartbeat
                            yield Ok(Event::default()
                                .comment(format!("heartbeat: {}", chrono::Utc::now())));
                        }
                    }
                    Err(e) => {
                        error!("Failed to poll run detail: {}", e);
                        yield Ok(Event::default()
                            .event("error")
                            .data(format!("{{\"error\": \"{}\"}}", e)));
                        break;
                    }
                    _ => {
                        // Run not found or JetStream unavailable
                        yield Ok(Event::default()
                            .event("error")
                            .data("{\"error\": \"Run not found\"}"));
                        break;
                    }
                }
            }
        } else {
            yield Ok(Event::default()
                .event("error")
                .data("{\"error\": \"JetStream not available\"}"));
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(heartbeat_seconds))
            .text("heartbeat"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jetstream::{RitualEvent, RunStatus};
    use std::collections::HashMap;

    #[test]
    fn test_run_summary_helpers() {
        let run = RunSummary {
            run_id: "test-run".to_string(),
            ritual_id: "test-ritual".to_string(),
            start_ts: chrono::Utc::now(),
            status: RunStatus::Completed,
        };

        assert_eq!(run.status_class(), "status-completed");
        assert!(run.format_timestamp().contains("UTC"));
    }

    #[test]
    fn test_ritual_event_helpers() {
        let event = RitualEvent {
            ts: chrono::Utc::now(),
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
            ts: chrono::Utc::now(),
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
            ts: chrono::Utc::now(),
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

    #[test]
    fn test_validate_query_params_valid_status() {
        let query = ListRunsQuery {
            status: Some("running".to_string()),
            capability: None,
            since: None,
            until: None,
            limit: None,
        };
        assert!(validate_query_params(&query).is_ok());

        let query = ListRunsQuery {
            status: Some("COMPLETED".to_string()), // case insensitive
            capability: None,
            since: None,
            until: None,
            limit: None,
        };
        assert!(validate_query_params(&query).is_ok());
    }

    #[test]
    fn test_validate_query_params_invalid_status() {
        let query = ListRunsQuery {
            status: Some("invalid".to_string()),
            capability: None,
            since: None,
            until: None,
            limit: None,
        };
        let result = validate_query_params(&query);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid status"));
    }

    #[test]
    fn test_validate_query_params_valid_timestamps() {
        // RFC3339 format
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: Some("2025-09-15T00:00:00Z".to_string()),
            until: Some("2025-09-15T23:59:59Z".to_string()),
            limit: None,
        };
        assert!(validate_query_params(&query).is_ok());

        // Unix timestamp format
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: Some("1726358400".to_string()),
            until: Some("1726444799".to_string()),
            limit: None,
        };
        assert!(validate_query_params(&query).is_ok());
    }

    #[test]
    fn test_validate_query_params_invalid_timestamps() {
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: Some("invalid-timestamp".to_string()),
            until: None,
            limit: None,
        };
        let result = validate_query_params(&query);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid since timestamp"));
    }

    #[test]
    fn test_validate_query_params_valid_limit() {
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: None,
            until: None,
            limit: Some(50),
        };
        assert!(validate_query_params(&query).is_ok());
    }

    #[test]
    fn test_validate_query_params_invalid_limit() {
        // Zero limit
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: None,
            until: None,
            limit: Some(0),
        };
        let result = validate_query_params(&query);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid limit"));

        // Too high limit
        let query = ListRunsQuery {
            status: None,
            capability: None,
            since: None,
            until: None,
            limit: Some(1000),
        };
        let result = validate_query_params(&query);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid limit"));
    }

    #[test]
    fn test_is_valid_timestamp() {
        // Valid RFC3339
        assert!(is_valid_timestamp("2025-09-15T00:00:00Z"));
        assert!(is_valid_timestamp("2025-09-15T12:30:45.123Z"));

        // Valid Unix seconds
        assert!(is_valid_timestamp("1726358400"));
        assert!(is_valid_timestamp("0"));

        // Invalid formats
        assert!(!is_valid_timestamp("invalid"));
        assert!(!is_valid_timestamp("2025-09-15"));
        assert!(!is_valid_timestamp("-1"));
    }
}
