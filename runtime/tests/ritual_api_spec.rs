use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use chrono::Utc;
use runtime::server::create_app_with_service;
use runtime::server::rituals::{
    AppPackRegistry, ExecutionPlan, RitualRunner, RitualService, RunStore,
};
use serde_json::json;
use tempfile::TempDir;
use tower::ServiceExt;

#[tokio::test]
async fn ritual_http_api_executes_and_persists_runs() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({
        "app": "hoss",
        "parameters": {"message": "hello"}
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("failed to build request"),
        )
        .await
        .expect("router error");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let run_id = created["runId"].as_str().expect("runId present");

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs?app=hoss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let runs = list_json["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["status"], "Completed");

    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/rituals/noop/runs/{}?app=hoss", run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail_json: serde_json::Value = serde_json::from_slice(&detail_body).unwrap();
    assert_eq!(detail_json["status"], "Completed");
    assert!(detail_json["resultEnvelope"].is_object());

    let envelope_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/rituals/noop/runs/{}/envelope?app=hoss",
                    run_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(envelope_response.status(), StatusCode::OK);
    let envelope_body = to_bytes(envelope_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let envelope_json: serde_json::Value = serde_json::from_slice(&envelope_body).unwrap();
    assert_eq!(envelope_json["runId"], run_id);
    assert!(envelope_json["envelope"].is_object());
}

#[tokio::test]
async fn ritual_http_api_validates_required_app_parameter() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({"parameters": {"message": "hello"}});

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn ritual_http_api_returns_error_for_unknown_app() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({
        "app": "unknown-app",
        "parameters": {}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(!response.status().is_success());
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].is_string());
}

#[tokio::test]
async fn ritual_http_api_returns_error_for_unknown_ritual() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({
        "app": "hoss",
        "parameters": {}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/unknown-ritual/runs")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(!response.status().is_success());
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].is_string());
}

#[tokio::test]
async fn ritual_http_api_list_filters_by_status() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({
        "app": "hoss",
        "parameters": {"message": "test"}
    });

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rituals/noop/runs")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs?app=hoss&status=Completed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let runs = list["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);

    let pending_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs?app=hoss&status=Pending")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = to_bytes(pending_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn ritual_http_api_list_respects_limit() {
    let (app, _tempdir) = setup_test_app().await;

    let payload = json!({
        "app": "hoss",
        "parameters": {}
    });

    for _ in 0..3 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/rituals/noop/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs?app=hoss&limit=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let list: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list["runs"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn ritual_http_api_detail_returns_404_for_nonexistent_run() {
    let (app, _tempdir) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs/nonexistent-id?app=hoss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn ritual_http_api_envelope_returns_404_for_nonexistent_run() {
    let (app, _tempdir) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs/nonexistent-id/envelope?app=hoss")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].as_str().unwrap().contains("not available"));
}

#[tokio::test]
async fn ritual_http_api_rejects_invalid_status_filter() {
    let (app, _tempdir) = setup_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/rituals/noop/runs?app=hoss&status=InvalidStatus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(error["error"].as_str().unwrap().contains("Invalid status"));
}

async fn setup_test_app() -> (axum::Router, TempDir) {
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
                "installed_at": Utc::now().to_rfc3339(),
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
    let service = RitualService::with_dependencies(registry, run_store, Arc::new(StubRunner));
    let router = create_app_with_service(Arc::new(service));

    (router, tempdir)
}

#[derive(Debug, Clone)]
struct StubRunner;

#[async_trait]
impl RitualRunner for StubRunner {
    async fn run(&self, plan: ExecutionPlan) -> anyhow::Result<serde_json::Value> {
        Ok(json!({
            "event": "ritual.completed:v1",
            "ritualId": plan.ritual_id,
            "runId": plan.run_id,
            "ts": Utc::now().to_rfc3339(),
            "outputs": {"result": "ok"}
        }))
    }
}
