use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt; // for oneshot

#[tokio::test]
async fn admin_probe_open_when_no_token() {
    std::env::remove_var("ADMIN_TOKEN");
    // Call the router directly
    let app = operate_ui::create_app(operate_ui::AppState::new().await);
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
    std::env::set_var("ADMIN_TOKEN", "secret");
    let app = operate_ui::create_app(operate_ui::AppState::new().await);
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
