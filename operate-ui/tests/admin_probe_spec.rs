use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt; // for oneshot

#[tokio::test]
async fn admin_probe_open_when_no_token() {
    std::env::remove_var("ADMIN_TOKEN");
    // Create AppState directly to ensure clean environment
    let jetstream_client = None; // No JetStream for this test
    let tera = tera::Tera::new("nonexistent/*").unwrap(); // Empty Tera
    let state = operate_ui::AppState {
        jetstream_client,
        tera,
        admin_token: None, // Explicitly no admin token
        bundle_loader: runtime::bundle::BundleLoader::new(None),
    };
    let app = operate_ui::create_app(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/admin/templates/report")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_probe_requires_token_when_set() {
    // Create AppState with explicit admin token
    let jetstream_client = None; // No JetStream for this test
    let tera = tera::Tera::new("nonexistent/*").unwrap(); // Empty Tera
    let state = operate_ui::AppState {
        jetstream_client,
        tera,
        admin_token: Some("secret".to_string()), // Explicitly set admin token
        bundle_loader: runtime::bundle::BundleLoader::new(None),
    };
    let app = operate_ui::create_app(state);
    // missing token -> 401
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/templates/report")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // correct token -> 200
    let resp_ok = app
        .oneshot(
            Request::builder()
                .uri("/admin/templates/report")
                .header("X-Admin-Token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_ok.status(), StatusCode::OK);
}
