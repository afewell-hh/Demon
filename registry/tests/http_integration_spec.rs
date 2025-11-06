//! HTTP endpoint integration tests
//!
//! These tests spin up a test server and verify HTTP endpoints.
//! Requires NATS server running with JetStream enabled.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use demon_registry::{create_app, kv::ContractBundle, AppState};
use tower::util::ServiceExt; // for `oneshot`

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_server_when_healthz_requested_then_returns_ok() -> Result<()> {
    // Arrange
    configure_env();
    let state = AppState::new().await?;
    let app = create_app(state);

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_stored_contract_when_http_get_requested_then_returns_bundle() -> Result<()> {
    // Arrange
    configure_env();
    let state = AppState::new().await?;
    let test_name = format!("http-test-{}", uuid::Uuid::new_v4());

    let bundle = ContractBundle {
        name: test_name.clone(),
        version: "1.0.0".to_string(),
        description: Some("HTTP test contract".to_string()),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        json_schema: Some(r#"{"type": "string"}"#.to_string()),
        wit_path: Some("/test.wit".to_string()),
        descriptor_path: Some("/test.json".to_string()),
        digest: Some("abc123".to_string()),
    };

    // Store contract via KV client directly
    state.kv_client.put_contract(&bundle).await?;

    let app = create_app(state.clone());

    // Act - Request via HTTP
    let uri = format!("/registry/contracts/{}/1.0.0", test_name);
    let response = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("Authorization", "Bearer dummy-token-for-testing")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    // Cleanup
    state.kv_client.delete_contract(&test_name, "1.0.0").await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_no_contracts_when_list_requested_then_returns_json_array() -> Result<()> {
    // Arrange
    configure_env();
    let state = AppState::new().await?;
    let app = create_app(state);

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .uri("/registry/contracts")
                .header("Authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_nonexistent_contract_when_requested_then_returns_404() -> Result<()> {
    // Arrange
    configure_env();
    let state = AppState::new().await?;
    let app = create_app(state);

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .uri("/registry/contracts/nonexistent/99.99.99")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;

    // Assert
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

fn configure_env() {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    std::env::set_var("NATS_URL", url);
    let bucket = format!("contracts_test_{}", uuid::Uuid::new_v4());
    std::env::set_var("REGISTRY_KV_BUCKET", bucket);
    // Set JWT_SECRET for authentication tests (required after security fix)
    std::env::set_var("JWT_SECRET", "test-secret-for-integration-tests");
}
