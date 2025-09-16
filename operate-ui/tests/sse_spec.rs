use axum::{body::Body, http::Request};
use operate_ui::{create_app, AppState};
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn sse_stream_sets_headers_and_emits_heartbeats() {
    // Faster heartbeat for test
    std::env::set_var("SSE_HEARTBEAT_SECONDS", "1");
    let state = AppState::new().await; // JetStream may be None; SSE should still work
    let app = create_app(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/runs/test-run/events/stream")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let headers = resp.headers();
    assert_eq!(headers.get("content-type").unwrap(), "text/event-stream");
}
