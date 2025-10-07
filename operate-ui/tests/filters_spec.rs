use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt; // for oneshot

#[tokio::test]
async fn list_runs_api_rejects_invalid_status() {
    // App with no JetStream; validation should still run first
    let state = operate_ui::AppState {
        jetstream_client: None,
        tera: tera::Tera::new("nonexistent/*").unwrap(),
        admin_token: None,
        bundle_loader: runtime::bundle::BundleLoader::new(None),
        app_pack_registry: None,
    };
    let app = operate_ui::create_app(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/runs?status=Bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_runs_api_rejects_invalid_limit() {
    let state = operate_ui::AppState {
        jetstream_client: None,
        tera: tera::Tera::new("nonexistent/*").unwrap(),
        admin_token: None,
        bundle_loader: runtime::bundle::BundleLoader::new(None),
        app_pack_registry: None,
    };
    let app = operate_ui::create_app(state);
    for bad in [0usize, 1001usize] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/runs?limit={}", bad))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
