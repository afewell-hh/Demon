//! Integration tests for POST /registry/contracts endpoint

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono;
use demon_registry::{auth::Claims, create_app, AppState};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::json;
use tower::ServiceExt;

/// Helper to create a test JWT token
fn create_test_token(scopes: Vec<String>, secret: &str) -> String {
    let claims = Claims {
        sub: "test-user".to_string(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        iat: Some(chrono::Utc::now().timestamp() as usize),
        scopes,
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_publish_contract_success() {
    // Set JWT secret for testing
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");

    // Create app state
    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    // Create token with contracts:write scope
    let token = create_test_token(vec!["contracts:write".to_string()], "test-secret");

    // Create request payload
    let payload = json!({
        "name": "test-contract",
        "version": "1.0.0",
        "description": "Test contract",
        "jsonSchema": r#"{"type": "object"}"#
    });

    // Make POST request
    let request = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body["status"], "created");
    assert_eq!(body["name"], "test-contract");
    assert_eq!(body["version"], "1.0.0");
    assert!(body["digest"].is_string());
    assert!(body["createdAt"].is_string());
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_publish_contract_missing_auth() {
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");

    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    let payload = json!({
        "name": "test-contract",
        "version": "1.0.0"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_publish_contract_invalid_scope() {
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");

    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    // Create token WITHOUT contracts:write scope
    let token = create_test_token(vec!["contracts:read".to_string()], "test-secret");

    let payload = json!({
        "name": "test-contract",
        "version": "1.0.0"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_publish_contract_duplicate_version() {
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");

    let state = AppState::new().await.expect("Failed to create app state");
    let app_1 = create_app(state.clone());
    let app_2 = create_app(state);

    let token = create_test_token(vec!["contracts:write".to_string()], "test-secret");

    let payload = json!({
        "name": "duplicate-test",
        "version": "1.0.0",
        "description": "Duplicate test"
    });

    // First request should succeed
    let request_1 = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response_1 = app_1.oneshot(request_1).await.unwrap();
    assert_eq!(response_1.status(), StatusCode::CREATED);

    // Second request with same name/version should fail with CONFLICT
    let token_2 = create_test_token(vec!["contracts:write".to_string()], "test-secret");
    let request_2 = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Authorization", format!("Bearer {}", token_2))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let response_2 = app_2.oneshot(request_2).await.unwrap();
    assert_eq!(response_2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
#[ignore] // Requires NATS JetStream
async fn test_publish_contract_invalid_json() {
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("NATS_URL", "nats://127.0.0.1:4222");

    let state = AppState::new().await.expect("Failed to create app state");
    let app = create_app(state);

    let token = create_test_token(vec!["contracts:write".to_string()], "test-secret");

    // Send invalid JSON
    let request = Request::builder()
        .method("POST")
        .uri("/registry/contracts")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
