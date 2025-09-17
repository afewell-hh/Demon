use crate::jetstream::{RunDetail, RunSummary};
use crate::{AppError, AppState};

use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, warn};

// Query parameters for list runs API
#[derive(Deserialize, Debug, Clone)]
pub struct ListRunsQuery {
    pub limit: Option<usize>,
    #[serde(rename = "ritual")]
    pub ritual_filter: Option<String>,
    #[serde(rename = "runId")]
    pub run_id_filter: Option<String>,
    pub status: Option<String>, // Running | Completed | Failed
}

fn parse_status_filter(s: &str) -> Option<crate::jetstream::RunStatus> {
    match s.to_ascii_lowercase().as_str() {
        "running" => Some(crate::jetstream::RunStatus::Running),
        "completed" => Some(crate::jetstream::RunStatus::Completed),
        "failed" => Some(crate::jetstream::RunStatus::Failed),
        _ => None,
    }
}

/// List runs - HTML response
#[axum::debug_handler]
pub async fn list_runs_html(
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Html<String> {
    debug!("Handling HTML list runs: {:?}", query);

    let (runs, error) = match &state.jetstream_client {
        Some(client) => match client.list_runs(query.limit).await {
            Ok(mut runs) => {
                // Apply in-memory filters (fast, bounded by limit)
                if let Some(ref r) = query.ritual_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.ritual_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref r) = query.run_id_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.run_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref s) = query.status {
                    if let Some(want) = parse_status_filter(s) {
                        runs.retain(|x| x.status == want);
                    }
                }
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
    context.insert("current_page", &"runs");
    // Reflect current filters in the template for persistence helpers
    context.insert("ritual_filter", &query.ritual_filter);
    context.insert("run_id_filter", &query.run_id_filter);
    context.insert("status_filter", &query.status);

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
    debug!("Handling JSON API list runs: {:?}", query);

    // Validate inputs
    if let Some(limit) = query.limit {
        if limit == 0 || limit > 1000 {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid 'limit': must be 1..=1000"
                })),
            )
                .into_response();
        }
    }
    if let Some(ref s) = query.status {
        if parse_status_filter(s).is_none() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid 'status': expected one of Running, Completed, Failed"
                })),
            )
                .into_response();
        }
    }

    match &state.jetstream_client {
        Some(client) => match client.list_runs(query.limit).await {
            Ok(mut runs) => {
                if let Some(ref r) = query.ritual_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.ritual_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref r) = query.run_id_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.run_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref s) = query.status {
                    if let Some(want) = parse_status_filter(s) {
                        runs.retain(|x| x.status == want);
                    }
                }
                info!("Successfully retrieved {} runs for API", runs.len());
                Json(runs).into_response()
            }
            Err(e) => {
                error!("Failed to retrieve runs: {}", e);
                (
                    StatusCode::BAD_GATEWAY,
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

/// SSE: Stream live events for a run from JetStream, with heartbeat fallback
pub async fn stream_run_events_sse(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Response {
    let hb_secs: u64 = std::env::var("SSE_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);

    // Try to get JetStream client
    let jetstream_client = if state.jetstream_client.is_some() {
        state.jetstream_client.clone()
    } else {
        // Try to create a new client if not available
        match crate::jetstream::JetStreamClient::new().await {
            Ok(client) => Some(client),
            Err(e) => {
                error!("Failed to connect to JetStream for SSE: {}", e);
                None
            }
        }
    };

    let run_id_owned = run_id.clone();
    let body_stream = async_stream::stream! {
        if let Some(js_client) = jetstream_client {
            // Stream with real events from JetStream
            match js_client.stream_run_events(&run_id_owned).await {
                Ok(event_stream) => {
                    // Send initial snapshot event
                    let init_payload = serde_json::json!({
                        "type": "init",
                        "runId": &run_id_owned,
                        "message": "Connected to event stream"
                    });
                    yield Ok::<_, std::io::Error>(
                        format!("event: init\ndata: {}\n\n", init_payload)
                    );

                    // Set up heartbeat timer
                    let interval = tokio::time::interval(Duration::from_secs(hb_secs.max(1)));
                    let mut heartbeat_stream = IntervalStream::new(interval);
                    let mut event_stream = Box::pin(event_stream.fuse());
                    let mut seq = 0u64;

                    // Multiplex events and heartbeats
                    loop {
                        tokio::select! {
                            // Real events from JetStream
                            Some(event_result) = event_stream.next() => {
                                match event_result {
                                    Ok(event) => {
                                        let payload = serde_json::json!({
                                            "type": "event",
                                            "runId": &run_id_owned,
                                            "event": event,
                                        });
                                        yield Ok(format!("event: append\ndata: {}\n\n", payload));
                                    }
                                    Err(e) => {
                                        warn!("Error streaming event: {}", e);
                                        // Continue streaming, don't break on errors
                                    }
                                }
                            }
                            // Heartbeats for liveness
                            Some(_) = heartbeat_stream.next() => {
                                let payload = serde_json::json!({
                                    "type": "heartbeat",
                                    "runId": &run_id_owned,
                                    "seq": seq
                                });
                                seq += 1;
                                yield Ok(format!("event: heartbeat\ndata: {}\n\n", payload));
                            }
                            // Both streams ended
                            else => break,
                        }
                    }
                }
                Err(e) => {
                    // Send error notification and fall back to heartbeat-only
                    error!("Failed to start event stream: {}", e);
                    let error_payload = serde_json::json!({
                        "type": "stream-error",
                        "runId": &run_id_owned,
                        "message": "Failed to connect to event stream, falling back to heartbeats"
                    });
                    yield Ok::<_, std::io::Error>(
                        format!("event: stream-error\ndata: {}\n\n", error_payload)
                    );

                    // Fall back to heartbeat-only mode
                    let interval = tokio::time::interval(Duration::from_secs(hb_secs.max(1)));
                    let mut ticks = IntervalStream::new(interval).enumerate();
                    while let Some((i, _)) = ticks.next().await {
                        let payload = serde_json::json!({
                            "type": "heartbeat",
                            "runId": &run_id_owned,
                            "seq": i as u64
                        });
                        yield Ok(format!("event: heartbeat\ndata: {}\n\n", payload));
                    }
                }
            }
        } else {
            // No JetStream available, heartbeat-only mode with warning
            let warning_payload = serde_json::json!({
                "type": "warning",
                "runId": &run_id_owned,
                "message": "JetStream unavailable, streaming heartbeats only"
            });
            yield Ok::<_, std::io::Error>(
                format!("event: warning\ndata: {}\n\n", warning_payload)
            );

            let interval = tokio::time::interval(Duration::from_secs(hb_secs.max(1)));
            let mut ticks = IntervalStream::new(interval).enumerate();
            while let Some((i, _)) = ticks.next().await {
                let payload = serde_json::json!({
                    "type": "heartbeat",
                    "runId": &run_id_owned,
                    "seq": i as u64
                });
                yield Ok(format!("event: heartbeat\ndata: {}\n\n", payload));
            }
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        axum::http::header::CONNECTION,
        axum::http::HeaderValue::from_static("keep-alive"),
    );
    (headers, axum::body::Body::from_stream(body_stream)).into_response()
}
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishOutcome {
    Published,
    Conflict,
}

async fn publish_approval_event(
    ritual_id: &str,
    run_id: &str,
    payload: serde_json::Value,
    msg_id: String,
    expected_stream_sequence: Option<u64>,
) -> anyhow::Result<PublishOutcome> {
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
    if let Some(seq) = expected_stream_sequence {
        let expected_header = seq.to_string();
        headers.insert(
            "Nats-Expected-Last-Subject-Sequence",
            expected_header.as_str(),
        );
    }
    match js
        .publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await
    {
        Ok(_) => Ok(PublishOutcome::Published),
        Err(err)
            if err.kind()
                == async_nats::jetstream::context::PublishErrorKind::WrongLastSequence =>
        {
            Ok(PublishOutcome::Conflict)
        }
        Err(err) => Err(err.into()),
    }
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
    headers: HeaderMap,
    Json(body): Json<ApproveBody>,
) -> Response {
    // CSRF protection: require X-Requested-With header for API calls
    if headers.get("X-Requested-With").is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "X-Requested-With header required"
            })),
        )
            .into_response();
    }
    if !approver_allowed(&body.approver) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "approver not allowed" })),
        )
            .into_response();
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
    let (ritual_id, expected_sequence) = match &state.jetstream_client {
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
                (
                    rd.ritual_id,
                    rd.events.last().and_then(|evt| evt.stream_sequence),
                )
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "run not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                error!("get_run_detail failed: {}", e);
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "JetStream error" })),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "JetStream unavailable" })),
            )
                .into_response()
        }
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
    match publish_approval_event(
        payload["ritualId"].as_str().unwrap(),
        payload["runId"].as_str().unwrap(),
        payload.clone(),
        msg_id,
        expected_sequence,
    )
    .await
    {
        Ok(PublishOutcome::Published) => (StatusCode::OK, Json(payload)).into_response(),
        Ok(PublishOutcome::Conflict) => {
            // Attempt to refresh state to report the final gate disposition
            let state_info = match &state.jetstream_client {
                Some(js) => js
                    .get_run_detail(&run_id)
                    .await
                    .ok()
                    .and_then(|opt| opt)
                    .and_then(|rd| {
                        rd.events.iter().rev().find_map(|evt| {
                            if (evt.event == "approval.granted:v1"
                                || evt.event == "approval.denied:v1")
                                && evt
                                    .extra
                                    .get("gateId")
                                    .and_then(|v| v.as_str())
                                    .map(|g| g == gate_id)
                                    .unwrap_or(false)
                            {
                                Some(if evt.event == "approval.granted:v1" {
                                    "granted"
                                } else {
                                    "denied"
                                })
                            } else {
                                None
                            }
                        })
                    }),
                None => None,
            };
            let mut body = serde_json::json!({
                "error": "approval write conflict",
                "hint": "refresh run timeline",
            });
            if let Some(state) = state_info {
                body["state"] = serde_json::Value::String(state.to_string());
            }
            (StatusCode::CONFLICT, Json(body)).into_response()
        }
        Err(e) => {
            error!("failed to publish: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("publish failed: {}", e)})),
            )
                .into_response()
        }
    }
}

#[axum::debug_handler]
pub async fn deny_approval_api(
    State(state): State<AppState>,
    Path((run_id, gate_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<DenyBody>,
) -> Response {
    // CSRF protection: require X-Requested-With header for API calls
    if headers.get("X-Requested-With").is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "X-Requested-With header required"
            })),
        )
            .into_response();
    }
    if !approver_allowed(&body.approver) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "approver not allowed" })),
        )
            .into_response();
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

    let (ritual_id, expected_sequence) = match &state.jetstream_client {
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
                (
                    rd.ritual_id,
                    rd.events.last().and_then(|evt| evt.stream_sequence),
                )
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "run not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                error!("get_run_detail failed: {}", e);
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": "JetStream error" })),
                )
                    .into_response();
            }
        },
        None => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "JetStream unavailable" })),
            )
                .into_response()
        }
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
    match publish_approval_event(
        payload["ritualId"].as_str().unwrap(),
        payload["runId"].as_str().unwrap(),
        payload.clone(),
        msg_id,
        expected_sequence,
    )
    .await
    {
        Ok(PublishOutcome::Published) => (StatusCode::OK, Json(payload)).into_response(),
        Ok(PublishOutcome::Conflict) => {
            let state_info = match &state.jetstream_client {
                Some(js) => js
                    .get_run_detail(&run_id)
                    .await
                    .ok()
                    .and_then(|opt| opt)
                    .and_then(|rd| {
                        rd.events.iter().rev().find_map(|evt| {
                            if (evt.event == "approval.granted:v1"
                                || evt.event == "approval.denied:v1")
                                && evt
                                    .extra
                                    .get("gateId")
                                    .and_then(|v| v.as_str())
                                    .map(|g| g == gate_id)
                                    .unwrap_or(false)
                            {
                                Some(if evt.event == "approval.granted:v1" {
                                    "granted"
                                } else {
                                    "denied"
                                })
                            } else {
                                None
                            }
                        })
                    }),
                None => None,
            };
            let mut body = serde_json::json!({
                "error": "approval write conflict",
                "hint": "refresh run timeline",
            });
            if let Some(state) = state_info {
                body["state"] = serde_json::Value::String(state.to_string());
            }
            (StatusCode::CONFLICT, Json(body)).into_response()
        }
        Err(e) => {
            error!("failed to publish: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": format!("publish failed: {}", e)})),
            )
                .into_response()
        }
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
            stream_sequence: None,
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
            stream_sequence: None,
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
            stream_sequence: None,
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

// Tenant-aware route handlers

/// List runs for a specific tenant - JSON API response
#[axum::debug_handler]
pub async fn list_runs_api_tenant(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<ListRunsQuery>,
) -> Response {
    debug!("Handling tenant {} JSON API list runs: {:?}", tenant, query);

    // Validate inputs
    if let Some(limit) = query.limit {
        if limit == 0 || limit > 1000 {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid 'limit': must be 1..=1000"
                })),
            )
                .into_response();
        }
    }
    if let Some(ref s) = query.status {
        if parse_status_filter(s).is_none() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid 'status': expected one of Running, Completed, Failed"
                })),
            )
                .into_response();
        }
    }

    match &state.jetstream_client {
        Some(client) => match client.list_runs_for_tenant(&tenant, query.limit).await {
            Ok(mut runs) => {
                if let Some(ref r) = query.ritual_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.ritual_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref r) = query.run_id_filter {
                    let needle = r.to_ascii_lowercase();
                    runs.retain(|x| x.run_id.to_ascii_lowercase().contains(&needle));
                }
                if let Some(ref s) = query.status {
                    if let Some(want) = parse_status_filter(s) {
                        runs.retain(|x| x.status == want);
                    }
                }
                info!(
                    "Successfully retrieved {} runs for tenant {} API",
                    runs.len(),
                    tenant
                );
                Json(serde_json::json!({ "runs": runs })).into_response()
            }
            Err(e) => {
                error!("Failed to retrieve runs for tenant {}: {}", tenant, e);
                (
                    StatusCode::BAD_GATEWAY,
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
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "JetStream is not available"
                })),
            )
                .into_response()
        }
    }
}

/// Get run detail for a specific tenant - JSON API response
#[axum::debug_handler]
pub async fn get_run_api_tenant(
    State(state): State<AppState>,
    Path((tenant, run_id)): Path<(String, String)>,
) -> Response {
    debug!(
        "Handling tenant {} JSON API request for run detail: {}",
        tenant, run_id
    );

    match &state.jetstream_client {
        Some(client) => match client.get_run_detail_for_tenant(&tenant, &run_id).await {
            Ok(Some(run)) => {
                info!(
                    "Successfully retrieved run detail for tenant {} API: {}",
                    tenant, run_id
                );
                Json(run).into_response()
            }
            Ok(None) => {
                info!("Run not found for tenant {}: {}", tenant, run_id);
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": "Run not found",
                        "tenantId": tenant,
                        "runId": run_id
                    })),
                )
                    .into_response()
            }
            Err(e) => {
                error!("Failed to retrieve run detail for tenant {}: {}", tenant, e);
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

/// Stream run events for a specific tenant - SSE response
#[axum::debug_handler]
pub async fn stream_run_events_sse_tenant(
    State(state): State<AppState>,
    Path((tenant, run_id)): Path<(String, String)>,
) -> Response {
    let hb_secs: u64 = std::env::var("SSE_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);

    // Try to get JetStream client
    let jetstream_client = if state.jetstream_client.is_some() {
        state.jetstream_client.clone()
    } else {
        // Try to create a new client if not available
        match crate::jetstream::JetStreamClient::new().await {
            Ok(client) => Some(client),
            Err(e) => {
                error!("Failed to connect to JetStream for SSE: {}", e);
                None
            }
        }
    };

    let tenant_owned = tenant.clone();
    let run_id_owned = run_id.clone();
    let body_stream = async_stream::stream! {
        if let Some(js_client) = jetstream_client {
            // Stream with real events from JetStream
            match js_client.stream_run_events_for_tenant(&tenant_owned, &run_id_owned).await {
                Ok(event_stream) => {
                    // Send initial snapshot event
                    let init_payload = serde_json::json!({
                        "type": "init",
                        "message": "Connected to event stream"
                    });
                    yield Ok::<_, std::convert::Infallible>(
                        axum::response::sse::Event::default()
                            .event("init")
                            .json_data(init_payload)
                            .expect("Valid JSON")
                    );

                    // Forward events from stream
                    futures_util::pin_mut!(event_stream);
                    let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(hb_secs));
                    heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                    loop {
                        tokio::select! {
                            event_result = event_stream.next() => {
                                match event_result {
                                    Some(Ok(event)) => {
                                        yield Ok(
                                            axum::response::sse::Event::default()
                                                .event("append")
                                                .json_data(event)
                                                .expect("Valid JSON")
                                        );
                                    }
                                    Some(Err(e)) => {
                                        error!("Stream error for tenant {} run {}: {}", tenant_owned, run_id_owned, e);
                                        let error_payload = serde_json::json!({
                                            "type": "error",
                                            "message": format!("Stream error: {}", e)
                                        });
                                        yield Ok(
                                            axum::response::sse::Event::default()
                                                .event("error")
                                                .json_data(error_payload)
                                                .expect("Valid JSON")
                                        );
                                        break;
                                    }
                                    None => {
                                        debug!("Event stream ended for tenant {} run {}", tenant_owned, run_id_owned);
                                        break;
                                    }
                                }
                            }
                            _ = heartbeat_interval.tick() => {
                                yield Ok(
                                    axum::response::sse::Event::default()
                                        .event("heartbeat")
                                        .comment("keep-alive")
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to create event stream for tenant {} run {}: {}", tenant_owned, run_id_owned, e);
                    let error_payload = serde_json::json!({
                        "type": "error",
                        "message": format!("Failed to create event stream: {}", e)
                    });
                    yield Ok::<_, std::convert::Infallible>(
                        axum::response::sse::Event::default()
                            .event("error")
                            .json_data(error_payload)
                            .expect("Valid JSON")
                    );
                }
            }
        } else {
            // JetStream not available - send warning and heartbeats
            let warning_payload = serde_json::json!({
                "type": "warning",
                "message": "JetStream unavailable; operating in degraded mode"
            });
            yield Ok::<_, std::convert::Infallible>(
                axum::response::sse::Event::default()
                    .event("warning")
                    .json_data(warning_payload)
                    .expect("Valid JSON")
            );

            // Send heartbeats only
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(hb_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                yield Ok(
                    axum::response::sse::Event::default()
                        .event("heartbeat")
                        .comment("keep-alive")
                );
            }
        }
    };

    axum::response::Sse::new(body_stream).into_response()
}

/// Grant approval for a specific tenant
#[axum::debug_handler]
pub async fn grant_approval_api_tenant(
    State(state): State<AppState>,
    Path((tenant, run_id, gate_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<ApproveBody>,
) -> Response {
    debug!(
        "Handling grant approval for tenant {} run {} gate {}",
        tenant, run_id, gate_id
    );

    // Delegate to existing grant_approval_api logic
    // For now, we'll just use the existing implementation
    grant_approval_api(State(state), Path((run_id, gate_id)), headers, Json(body)).await
}

/// Deny approval for a specific tenant
#[axum::debug_handler]
pub async fn deny_approval_api_tenant(
    State(state): State<AppState>,
    Path((tenant, run_id, gate_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Json(body): Json<DenyBody>,
) -> Response {
    debug!(
        "Handling deny approval for tenant {} run {} gate {}",
        tenant, run_id, gate_id
    );

    // Delegate to existing deny_approval_api logic
    // For now, we'll just use the existing implementation
    deny_approval_api(State(state), Path((run_id, gate_id)), headers, Json(body)).await
}
