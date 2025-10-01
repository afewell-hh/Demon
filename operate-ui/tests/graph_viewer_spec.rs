use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use operate_ui::{create_app, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_graph_viewer_page_loads() {
    let state = AppState::new().await;
    let app = create_app(state);

    let request = Request::builder()
        .uri("/graph")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Check that the page contains expected elements
    assert!(body_str.contains("Graph Viewer"));
    assert!(body_str.contains("tenantId"));
    assert!(body_str.contains("projectId"));
    assert!(body_str.contains("namespace"));
    assert!(body_str.contains("graphId"));
}

#[tokio::test]
async fn test_graph_viewer_with_query_params() {
    let state = AppState::new().await;
    let app = create_app(state);

    let request = Request::builder()
        .uri("/graph?tenantId=test-tenant&projectId=test-project&namespace=test-ns&graphId=test-graph")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Check that query parameters are reflected in the page
    assert!(body_str.contains("test-tenant"));
    assert!(body_str.contains("test-project"));
    assert!(body_str.contains("test-ns"));
    assert!(body_str.contains("test-graph"));
}
