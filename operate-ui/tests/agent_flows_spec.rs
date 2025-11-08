// Agent Flow API integration tests

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use operate_ui::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    sub: String,
    scope: String,
    exp: usize,
}

fn create_test_jwt(scopes: &str) -> String {
    let claims = TestClaims {
        sub: "test-agent".to_string(),
        scope: scopes.to_string(),
        exp: 9999999999, // Far future expiration
    };

    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "test-secret".to_string());
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("Failed to create test JWT")
}

fn create_test_app() -> axum::Router {
    let mut tera = tera::Tera::new("nonexistent/*").expect("Failed to create empty Tera instance");
    tera.add_raw_template("error.html", "<html><body>Error</body></html>")
        .expect("Failed to add error template");

    let state = AppState {
        jetstream_client: None,
        tera,
        admin_token: None,
        bundle_loader: runtime::bundle::BundleLoader::new(None),
        app_pack_registry: None,
    };

    operate_ui::create_app(state)
}

#[tokio::test]
#[serial]
async fn test_draft_flow_validation() {
    // Set JWT secret for auth
    std::env::set_var("JWT_SECRET", "test-secret");
    // Enable feature flag
    std::env::set_var("OPERATE_UI_FLAGS", "agent-flows");

    let app = create_test_app();
    let token = create_test_jwt("flows:write");

    let manifest = json!({
        "schema_version": "v1",
        "metadata": {
            "flow_id": "test-flow-001",
            "name": "Test Flow",
            "created_by": "test-agent"
        },
        "nodes": [],
        "edges": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/flows/draft")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(
                    serde_json::to_string(&json!({ "manifest": manifest })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Clean up
    std::env::remove_var("OPERATE_UI_FLAGS");
    std::env::remove_var("JWT_SECRET");
}

#[tokio::test]
#[serial]
async fn test_submit_flow_missing_schema_version() {
    // Set JWT secret for auth
    std::env::set_var("JWT_SECRET", "test-secret");
    std::env::set_var("OPERATE_UI_FLAGS", "agent-flows");

    let app = create_test_app();
    let token = create_test_jwt("flows:write");

    let manifest = json!({
        "metadata": {
            "flow_id": "test-flow-002",
            "name": "Invalid Flow",
            "created_by": "test-agent"
        },
        "nodes": [],
        "edges": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/flows/submit")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::from(
                    serde_json::to_string(&json!({ "manifest": manifest })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    std::env::remove_var("OPERATE_UI_FLAGS");
    std::env::remove_var("JWT_SECRET");
}
