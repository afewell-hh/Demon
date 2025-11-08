// Agent Flow API integration tests

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use operate_ui::AppState;
use serde_json::json;
use tower::ServiceExt;

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
async fn test_list_contracts_without_feature_flag() {
    // Ensure feature flag is not set
    std::env::remove_var("OPERATE_UI_FLAGS");

    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/contracts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_draft_flow_validation() {
    // Enable feature flag
    std::env::set_var("OPERATE_UI_FLAGS", "agent-flows");

    let app = create_test_app();

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
                .body(Body::from(serde_json::to_string(&json!({ "manifest": manifest })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Clean up
    std::env::remove_var("OPERATE_UI_FLAGS");
}

#[tokio::test]
async fn test_submit_flow_missing_schema_version() {
    std::env::set_var("OPERATE_UI_FLAGS", "agent-flows");

    let app = create_test_app();

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
                .body(Body::from(serde_json::to_string(&json!({ "manifest": manifest })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    std::env::remove_var("OPERATE_UI_FLAGS");
}
