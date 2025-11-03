//! Tests for /healthz endpoint (no auth required)

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use demon_registry::{create_app, AppState};
use tower::ServiceExt;

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_healthz_no_auth_required() {
    // Arrange
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");
    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    // Act - Call /healthz without Authorization header
    let request = Request::builder()
        .method("GET")
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Assert - Should return 200 OK without requiring auth
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert_eq!(body, "OK");
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_registry_routes_require_auth() {
    // Arrange
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");
    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    // Act - Call /registry/contracts without Authorization header
    let request = Request::builder()
        .method("GET")
        .uri("/registry/contracts")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Assert - Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
