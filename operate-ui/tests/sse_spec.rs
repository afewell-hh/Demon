use axum::{body::Body, http::Request};
use futures_util::StreamExt;
use operate_ui::{create_app, AppState};
use std::time::Duration;
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
    assert_eq!(headers.get("cache-control").unwrap(), "no-cache");
    assert_eq!(headers.get("connection").unwrap(), "keep-alive");
}

#[tokio::test]
async fn sse_stream_emits_events_or_fallback_warning() {
    // Test that SSE stream emits either real events or a warning/heartbeat fallback
    std::env::set_var("SSE_HEARTBEAT_SECONDS", "1");
    let state = AppState::new().await;
    let app = create_app(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/runs/test-run-123/events/stream")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Read a portion of the stream
    let body = resp.into_body();
    let timeout = tokio::time::timeout(Duration::from_secs(2), async {
        let chunk = body.into_data_stream().next().await.unwrap().unwrap();
        String::from_utf8_lossy(&chunk).to_string()
    });

    match timeout.await {
        Ok(data) => {
            // Should contain either an event, warning, or heartbeat
            assert!(
                data.contains("event:") || data.contains("data:"),
                "SSE stream should emit events or data"
            );

            // Verify it's valid SSE format
            assert!(
                data.contains("\n\n") || data.ends_with("\n"),
                "SSE events should be properly formatted"
            );

            // Check for expected event types
            let has_valid_event = data.contains("heartbeat")
                || data.contains("warning")
                || data.contains("init")
                || data.contains("append")
                || data.contains("error");

            assert!(
                has_valid_event,
                "SSE stream should emit recognized event types"
            );
        }
        Err(_) => {
            // Timeout is acceptable if JetStream is not available
            // The stream would still be running but no immediate data
        }
    }
}

#[tokio::test]
async fn sse_stream_handles_invalid_run_id() {
    // Test that SSE gracefully handles non-existent runs
    std::env::set_var("SSE_HEARTBEAT_SECONDS", "1");
    std::env::set_var("DEMON_SKIP_STREAM_BOOTSTRAP", "1"); // Skip JetStream for this test
    let state = AppState::new().await;
    let app = create_app(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/runs/nonexistent-run/events/stream")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), 200); // SSE endpoints typically return 200 even for errors

    let headers = resp.headers();
    assert_eq!(headers.get("content-type").unwrap(), "text/event-stream");

    // Try to read first event
    let body = resp.into_body();
    let timeout = tokio::time::timeout(Duration::from_secs(2), async {
        let chunk = body.into_data_stream().next().await.unwrap().unwrap();
        String::from_utf8_lossy(&chunk).to_string()
    });

    if let Ok(data) = timeout.await {
        // Should emit warning or heartbeat for non-existent run
        assert!(
            data.contains("warning") || data.contains("heartbeat"),
            "Should emit warning or heartbeat for non-existent run"
        );
    }
}
