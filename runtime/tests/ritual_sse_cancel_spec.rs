use std::sync::Arc;

use async_trait::async_trait;
use axum::body::to_bytes;
use axum::http::{Request, StatusCode};
use runtime::server::create_app_with_service;
use runtime::server::rituals::{
    AppPackRegistry, ExecutionPlan, RitualRunner, RitualService, RunStore,
};
use serde_json::json;
use tempfile::TempDir;
use tower::ServiceExt;

/// SSE for ritual runs should emit status updates and, when available, an envelope,
/// then terminate once the run reaches a terminal state.
#[tokio::test]
async fn ritual_sse_emits_status_and_envelope_then_ends() {
    let (app, _tmp) = setup_test_app_with_runner(Arc::new(FastRunner)).await;

    // Schedule a run that completes quickly
    let payload = json!({
        "app": "hoss",
        "parameters": {"message": "hello"}
    });

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    let run_id = created["runId"].as_str().unwrap().to_string();

    // Use small heartbeat to accelerate loop
    let sse_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/rituals/noop/runs/{}/events/stream?app=hoss&heartbeat_secs=1",
                    run_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(sse_resp.status(), StatusCode::OK);
    // Collect the whole SSE body (stream completes after terminal state)
    let body = to_bytes(sse_resp.into_body(), usize::MAX).await.unwrap();
    let s = String::from_utf8_lossy(&body);

    // Expect at least one status line and an envelope line
    assert!(
        s.contains("\"type\":\"status\""),
        "should emit status JSON events"
    );
    assert!(
        s.contains("\"type\":\"envelope\""),
        "should emit envelope JSON event at end"
    );
}

/// Cancel endpoint should move a running task to Canceled and SSE should not emit an envelope.
#[tokio::test]
async fn ritual_cancel_stops_run_and_sse_finishes_without_envelope() {
    let (app, _tmp) = setup_test_app_with_runner(Arc::new(SlowRunner)).await;

    // Schedule a slow run
    let payload = json!({
        "app": "hoss",
        "parameters": {}
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    let run_id = created["runId"].as_str().unwrap().to_string();

    // Immediately request cancel
    let cancel_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/rituals/noop/runs/{}/cancel?app=hoss",
                    run_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(cancel_resp.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(cancel_resp.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(json["canceled"], true);

    // Verify run detail reflects Canceled and has no envelope
    let detail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/rituals/noop/runs/{}?app=hoss", run_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let detail: serde_json::Value =
        serde_json::from_slice(&to_bytes(detail_resp.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(detail["status"], "Canceled");
    assert!(detail["resultEnvelope"].is_null());

    // SSE should emit status updates and finish without an envelope event
    let sse_resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/rituals/noop/runs/{}/events/stream?app=hoss&heartbeat_secs=1",
                    run_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sse_resp.status(), StatusCode::OK);
    let body = to_bytes(sse_resp.into_body(), usize::MAX).await.unwrap();
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("\"type\":\"status\""));
    assert!(
        !s.contains("\"type\":\"envelope\""),
        "canceled runs should not emit envelope event"
    );
}

/// When subscribed to SSE, issuing cancel should cause the stream to end
/// without emitting an envelope event.
#[tokio::test]
async fn ritual_sse_subscribe_then_cancel_finishes_without_envelope() {
    let (app, _tmp) = setup_test_app_with_runner(Arc::new(SlowRunner)).await;

    // Schedule a slow run
    let payload = json!({ "app": "hoss", "parameters": {} });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    let run_id = created["runId"].as_str().unwrap().to_string();

    // Start SSE subscription
    let sse_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/rituals/noop/runs/{}/events/stream?app=hoss&heartbeat_secs=1",
                    run_id
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sse_response.status(), StatusCode::OK);

    // Cancel shortly after subscribing
    let cancel_task_app = app.clone();
    let cancel_run_id = run_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let _ = cancel_task_app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/v1/rituals/noop/runs/{}/cancel?app=hoss",
                        cancel_run_id
                    ))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await;
    });

    // Read stream to completion
    let body = to_bytes(sse_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let s = String::from_utf8_lossy(&body);
    assert!(s.contains("\"type\":\"status\""));
    assert!(s.contains("Canceled") || s.contains("canceled"));
    assert!(
        !s.contains("\"type\":\"envelope\""),
        "should not emit envelope event when canceled mid-stream"
    );
}

async fn setup_test_app_with_runner(runner: Arc<dyn RitualRunner>) -> (axum::Router, TempDir) {
    let tempdir = tempfile::tempdir().unwrap();
    let app_root = tempdir.path().join("app-packs");
    std::fs::create_dir_all(app_root.clone()).unwrap();

    let packs_dir = app_root.join("packs").join("hoss").join("0.1.0");
    std::fs::create_dir_all(&packs_dir).unwrap();

    let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let manifest_src =
        std::fs::read_to_string(workspace_root.join("examples/app-packs/hoss/app-pack.yaml"))
            .unwrap();
    let manifest_path = packs_dir.join("app-pack.yaml");
    std::fs::write(&manifest_path, manifest_src).unwrap();

    let registry = json!({
        "apps": {
            "hoss": [{
                "version": "0.1.0",
                "manifest_path": manifest_path,
                "installed_at": chrono::Utc::now().to_rfc3339(),
                "source": "tests",
                "schema_range": ">=1.0.0 <2.0.0"
            }]
        }
    });
    std::fs::write(
        app_root.join("registry.json"),
        serde_json::to_string_pretty(&registry).unwrap(),
    )
    .unwrap();

    let run_store_path = tempdir.path().join("runtime").join("runs.json");
    let run_store = RunStore::open(run_store_path).unwrap();
    let registry = AppPackRegistry::with_root(app_root);
    let service = RitualService::with_dependencies(registry, run_store, runner);
    let router = create_app_with_service(Arc::new(service));

    (router, tempdir)
}

#[derive(Debug, Clone)]
struct FastRunner;

#[async_trait]
impl RitualRunner for FastRunner {
    async fn run(&self, plan: ExecutionPlan) -> anyhow::Result<serde_json::Value> {
        Ok(json!({
            "event": "ritual.completed:v1",
            "ritualId": plan.ritual_id,
            "runId": plan.run_id,
            "ts": chrono::Utc::now().to_rfc3339(),
            "outputs": {"result": "ok"}
        }))
    }
}

#[derive(Debug, Clone)]
struct SlowRunner;

#[async_trait]
impl RitualRunner for SlowRunner {
    async fn run(&self, plan: ExecutionPlan) -> anyhow::Result<serde_json::Value> {
        // Sleep long enough for cancel to trigger in tests
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(json!({
            "event": "ritual.completed:v1",
            "ritualId": plan.ritual_id,
            "runId": plan.run_id,
            "ts": chrono::Utc::now().to_rfc3339(),
            "outputs": {"result": "ok"}
        }))
    }
}
