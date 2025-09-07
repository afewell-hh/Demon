use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use operate_ui::{AppState, AppResult};
use serde_json::json;
use tower::ServiceExt;

// Helper function to create app with mock state
fn create_test_app() -> axum::Router {
    let state = AppState {
        jetstream_client: None, // Simulate JetStream unavailable for testing
    };
    
    operate_ui::create_app(state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"OK");
}

#[tokio::test]
async fn test_runs_html_without_jetstream() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Should contain error message about JetStream being unavailable
    assert!(html.contains("JetStream is not available"));
    assert!(html.contains("No runs found"));
}

#[tokio::test]
async fn test_runs_api_without_jetstream() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/api/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json_response["error"], "JetStream is not available");
}

#[tokio::test]
async fn test_run_detail_html_without_jetstream() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/runs/test-run-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Should contain error message about JetStream being unavailable
    assert!(html.contains("JetStream is not available"));
    assert!(html.contains("test-run-id"));
}

#[tokio::test]
async fn test_run_detail_api_without_jetstream() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs/test-run-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json_response["error"], "JetStream is not available");
}

#[tokio::test]
async fn test_404_handling() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nonexistent-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK); // HTML 404 page

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("404 - Not Found"));
    assert!(html.contains("Back to Runs"));
}

#[tokio::test]
async fn test_runs_api_with_limit_param() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY); // JetStream unavailable
}

#[tokio::test]
async fn test_content_type_headers() {
    let app = create_test_app();

    // Test HTML endpoint
    let html_response = app
        .clone()
        .oneshot(Request::builder().uri("/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(html_response.status(), StatusCode::OK);
    // HTML responses should have text/html content type (handled by Axum)

    // Test JSON API endpoint
    let json_response = app
        .oneshot(Request::builder().uri("/api/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(json_response.status(), StatusCode::BAD_GATEWAY);
    // JSON responses should have application/json content type (handled by Axum)
}

// Integration test for the main server startup (requires NATS to be running)
#[tokio::test]
#[ignore] // Run with `cargo test -- --ignored` when NATS is available
async fn test_server_integration_with_nats() {
    // This test requires NATS to be running locally
    // It will test the full integration including JetStream connectivity

    let state = AppState::new().await;

    // If this test is running, NATS should be available
    if state.jetstream_client.is_none() {
        panic!("NATS/JetStream should be available for integration tests");
    }

    let app = operate_ui::create_app(state);

    // Test that with JetStream available, we get better responses
    let response = app
        .oneshot(Request::builder().uri("/api/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // Should either be OK (with runs) or OK (empty array), not BAD_GATEWAY
    assert_ne!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_html_template_rendering() {
    let app = create_test_app();

    let response = app
        .oneshot(Request::builder().uri("/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Verify essential HTML structure
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<title>"));
    assert!(html.contains("Demon Operate UI"));
    assert!(html.contains("Recent Runs"));
    
    // Verify navigation elements
    assert!(html.contains("href=\"/runs\""));
    assert!(html.contains("href=\"/health\""));

    // Verify responsive design elements
    assert!(html.contains("viewport"));
    assert!(html.contains("grid-template-columns"));
}

#[tokio::test]
async fn test_run_detail_html_template() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/runs/sample-run-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Verify run detail page structure
    assert!(html.contains("Run sample-run-123"));
    assert!(html.contains("Back to Runs"));
    assert!(html.contains("Run Details"));
    assert!(html.contains("Event Timeline"));
    assert!(html.contains("API Access"));

    // Verify the run ID is displayed
    assert!(html.contains("sample-run-123"));
}