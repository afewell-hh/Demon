use axum_test::TestServer;
use operate_ui::{create_app, AppState};

#[tokio::test]
async fn test_workflow_viewer_page_loads() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/ui/workflow").await;
    assert_eq!(response.status_code(), 200);
    assert!(response.text().contains("Serverless Workflow Viewer"));
}

#[tokio::test]
async fn test_workflow_viewer_with_path() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/ui/workflow")
        .add_query_param("workflowPath", "echo.yaml")
        .await;

    assert_eq!(response.status_code(), 200);
    assert!(response.text().contains("Serverless Workflow Viewer"));
}

#[tokio::test]
async fn test_workflow_metadata_local() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/api/workflow/metadata")
        .add_query_param("workflowPath", "echo.yaml")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert!(body.get("workflow").is_some());
    assert!(body.get("workflowId").is_some());
    assert_eq!(body.get("source").and_then(|v| v.as_str()), Some("local"));

    // Verify workflow structure (legacy format)
    let workflow = body.get("workflow").unwrap();
    assert!(
        workflow.get("states").is_some() || workflow.get("document").is_some(),
        "Workflow should have states or document structure"
    );
}

#[tokio::test]
async fn test_workflow_metadata_missing_params() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server.get("/api/workflow/metadata").await;

    assert_eq!(response.status_code(), 400);

    let body: serde_json::Value = response.json();
    assert!(body.get("error").is_some());
    assert!(body
        .get("error")
        .and_then(|e| e.as_str())
        .unwrap()
        .contains("workflowUrl or workflowPath"));
}

#[tokio::test]
async fn test_workflow_metadata_nonexistent_local() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/api/workflow/metadata")
        .add_query_param("workflowPath", "nonexistent-workflow.yaml")
        .await;

    assert_eq!(response.status_code(), 404);

    let body: serde_json::Value = response.json();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn test_workflow_state_api() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    let response = server
        .get("/api/workflow/state")
        .add_query_param("workflowId", "test-workflow")
        .await;

    assert_eq!(response.status_code(), 200);

    let body: serde_json::Value = response.json();
    assert!(body.get("workflowId").is_some());
    assert!(body.get("currentState").is_some());
    assert_eq!(
        body.get("workflowId").and_then(|v| v.as_str()),
        Some("test-workflow")
    );
}

#[tokio::test]
async fn test_workflow_metadata_path_traversal_protection() {
    let state = AppState::new().await;
    let app = create_app(state);
    let server = TestServer::new(app).unwrap();

    // Attempt path traversal
    let response = server
        .get("/api/workflow/metadata")
        .add_query_param("workflowPath", "../../../etc/passwd")
        .await;

    // Should either fail or sanitize the path
    assert!(response.status_code() == 404 || response.status_code() == 400);
}
