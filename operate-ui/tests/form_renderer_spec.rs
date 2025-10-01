use axum_test::TestServer;
use operate_ui::{create_app, AppState};
use serde_json::json;

#[tokio::test]
async fn test_form_renderer_page_loads() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/ui/form").await;
    assert_eq!(response.status_code(), 200);
    assert!(response.text().contains("JSON Schema Form Renderer"));
}

#[tokio::test]
async fn test_form_renderer_with_schema_name() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/ui/form")
        .add_query_param("schemaName", "approval.requested.v1")
        .await;

    assert_eq!(response.status_code(), 200);
    assert!(response.text().contains("JSON Schema Form Renderer"));
}

#[tokio::test]
async fn test_schema_metadata_local() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/api/schema/metadata")
        .add_query_param("schemaName", "approval.requested.v1")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert!(body.get("schema").is_some());
    assert!(body.get("schemaId").is_some());
    assert_eq!(body.get("source").and_then(|v| v.as_str()), Some("local"));

    // Verify schema structure
    let schema = body.get("schema").unwrap();
    assert!(schema.get("properties").is_some());
}

#[tokio::test]
async fn test_schema_metadata_missing_params() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/api/schema/metadata").await;

    assert_eq!(response.status_code(), 400);

    let body: serde_json::Value = response.json();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn test_schema_metadata_nonexistent_local() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/api/schema/metadata")
        .add_query_param("schemaName", "nonexistent.schema.v1")
        .await;

    assert_eq!(response.status_code(), 404);

    let body: serde_json::Value = response.json();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn test_form_submit() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let payload = json!({
        "schemaId": "test-schema",
        "data": {
            "field1": "value1",
            "field2": 42
        }
    });

    let response = server.post("/api/form/submit").json(&payload).await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert_eq!(
        body.get("status").and_then(|v| v.as_str()),
        Some("received")
    );
    assert!(body.get("data").is_some());
}

#[tokio::test]
async fn test_form_changed_event_emission() {
    // This is a unit test for the JavaScript event emission logic
    // In a real scenario, you'd use a headless browser like Playwright
    // For now, we'll just verify the endpoint exists
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/ui/form").await;
    assert_eq!(response.status_code(), 200);

    // Verify the template includes form.changed event logic
    let html = response.text();
    assert!(html.contains("form.changed"));
    assert!(html.contains("emitFormChanged"));
}

#[tokio::test]
async fn test_accessible_form_structure() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/ui/form").await;
    assert_eq!(response.status_code(), 200);

    let html = response.text();

    // Check for accessibility features
    assert!(html.contains("role=\"form\""));
    assert!(html.contains("aria-live=\"polite\""));
    assert!(html.contains("label"));
}

#[tokio::test]
async fn test_schema_path_traversal_prevention() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    // Try path traversal attack
    let response = server
        .get("/api/schema/metadata")
        .add_query_param("schemaName", "../../../etc/passwd")
        .await;

    // Should fail to find the file (404) not expose system files
    assert_eq!(response.status_code(), 404);
}
