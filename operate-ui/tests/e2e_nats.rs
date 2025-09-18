use axum::Router;
use operate_ui::{create_app, AppState};
use std::str::FromStr;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};

#[tokio::test]
#[ignore] // Runs in CI after NATS is up
async fn runs_endpoints_behave_with_and_without_stream() -> anyhow::Result<()> {
    // Start server on an ephemeral port with bootstrap disabled to simulate missing stream
    std::env::set_var("DEMON_SKIP_STREAM_BOOTSTRAP", "1");
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let state = AppState::new().await;
    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // 1) No stream yet: expect empty runs and warning header
    let r = client.get(format!("{}/api/runs", base)).send().await?;
    assert_eq!(r.status(), 200);
    assert!(r.headers().get("X-Demon-Warn").is_some());
    let body = r.text().await?;
    assert!(body.contains("\"runs\":[]"));

    // 2) Ensure stream + publish fixtures via NATS
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let nc = async_nats::connect(nats_url).await?;
    let js = async_nats::jetstream::new(nc);
    let stream_name =
        std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let subjects = std::env::var("RITUAL_SUBJECTS").unwrap_or_else(|_| "demon.ritual.v1.>".into());

    if js.get_stream(&stream_name).await.is_err() {
        js.create_stream(async_nats::jetstream::stream::Config {
            name: stream_name.clone(),
            subjects: vec![subjects.clone()],
            duplicate_window: std::time::Duration::from_secs(120),
            ..Default::default()
        })
        .await?;
    }

    let subj = "demon.ritual.v1.e2e-ritual.e2e-run.events";
    // started
    let mut h1 = async_nats::HeaderMap::new();
    h1.insert(
        "Nats-Msg-Id",
        async_nats::HeaderValue::from_str("e2e-run:1").unwrap(),
    );
    js.publish_with_headers(
        subj,
        h1,
        serde_json::to_vec(&serde_json::json!({
            "event": "ritual.started:v1",
            "ritualId": "e2e-ritual",
            "runId": "e2e-run",
            "ts": "2025-01-01T00:00:00Z"
        }))?
        .into(),
    )
    .await?;
    // completed
    let mut h2 = async_nats::HeaderMap::new();
    h2.insert(
        "Nats-Msg-Id",
        async_nats::HeaderValue::from_str("e2e-run:2").unwrap(),
    );
    js.publish_with_headers(
        subj,
        h2,
        serde_json::to_vec(&serde_json::json!({
            "event": "ritual.completed:v1",
            "ritualId": "e2e-ritual",
            "runId": "e2e-run",
            "ts": "2025-01-01T00:00:05Z",
            "outputs": {
                "result": {
                    "success": true,
                    "data": {
                        "echoed_message": "Hello from test",
                        "character_count": 15,
                        "timestamp": "2025-01-01T00:00:05Z"
                    }
                },
                "diagnostics": [{
                    "level": "info",
                    "message": "Echo operation completed",
                    "timestamp": "2025-01-01T00:00:05Z"
                }],
                "metrics": {
                    "counters": {"characterCount": 15},
                    "duration": {"total_ms": 0.5}
                },
                "provenance": {
                    "source": {
                        "system": "echo-capsule",
                        "version": "0.0.1"
                    },
                    "timestamp": "2025-01-01T00:00:05Z"
                }
            }
        }))?
        .into(),
    )
    .await?;

    // Allow small delay for visibility
    sleep(Duration::from_millis(200)).await;

    // 2a) List shows the run
    let r2 = client.get(format!("{}/api/runs", base)).send().await?;
    assert_eq!(r2.status(), 200);
    let b2 = r2.text().await?;
    assert!(b2.contains("\"runId\":\"e2e-run\""));

    // 2b) Detail JSON contains both events
    let r3 = client
        .get(format!("{}/api/runs/{}", base, "e2e-run"))
        .send()
        .await?;
    assert_eq!(r3.status(), 200);
    let b3 = r3.text().await?;
    assert!(b3.contains("ritual.started:v1"));
    assert!(b3.contains("ritual.completed:v1"));

    // 2c) HTML pages render
    let h1 = client
        .get(format!("{}/runs", base))
        .send()
        .await?
        .text()
        .await?;
    assert!(h1.contains("Runs") || h1.contains("No event stream"));
    let h2 = client
        .get(format!("{}/runs/{}", base, "e2e-run"))
        .send()
        .await?
        .text()
        .await?;
    assert!(h2.contains("e2e-run"));

    Ok(())
}

#[tokio::test]
#[ignore] // Runs in CI after NATS is up
async fn stream_events_with_identical_timestamps_are_not_dropped() -> anyhow::Result<()> {
    // Start server on an ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let state = AppState::new().await;
    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Setup NATS and publish events with identical timestamps
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let nc = async_nats::connect(nats_url).await?;
    let js = async_nats::jetstream::new(nc);
    let stream_name =
        std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let subjects = std::env::var("RITUAL_SUBJECTS").unwrap_or_else(|_| "demon.ritual.v1.>".into());

    if js.get_stream(&stream_name).await.is_err() {
        js.create_stream(async_nats::jetstream::stream::Config {
            name: stream_name.clone(),
            subjects: vec![subjects.clone()],
            duplicate_window: std::time::Duration::from_secs(120),
            ..Default::default()
        })
        .await?;
    }

    let subj = "demon.ritual.v1.test-ritual.test-run-dup.events";
    let identical_ts = "2025-01-06T10:30:02Z";

    // Publish multiple events with identical timestamps
    let mut h1 = async_nats::HeaderMap::new();
    h1.insert(
        "Nats-Msg-Id",
        async_nats::HeaderValue::from_str("test-run-dup:1").unwrap(),
    );
    js.publish_with_headers(
        subj,
        h1,
        serde_json::to_vec(&serde_json::json!({
            "event": "policy.decision.allowed:v1",
            "ritualId": "test-ritual",
            "runId": "test-run-dup",
            "ts": identical_ts,
            "policyId": "test-policy"
        }))?
        .into(),
    )
    .await?;

    let mut h2 = async_nats::HeaderMap::new();
    h2.insert(
        "Nats-Msg-Id",
        async_nats::HeaderValue::from_str("test-run-dup:2").unwrap(),
    );
    js.publish_with_headers(
        subj,
        h2,
        serde_json::to_vec(&serde_json::json!({
            "event": "ritual.completed:v1",
            "ritualId": "test-ritual",
            "runId": "test-run-dup",
            "ts": identical_ts,  // Same timestamp as previous event
            "outputs": {"result": "success"}
        }))?
        .into(),
    )
    .await?;

    // Allow small delay for visibility
    sleep(Duration::from_millis(200)).await;

    // Fetch run detail and verify both events are present
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);
    let r = client
        .get(format!("{}/api/runs/{}", base, "test-run-dup"))
        .send()
        .await?;
    assert_eq!(r.status(), 200);

    let body = r.text().await?;
    let json: serde_json::Value = serde_json::from_str(&body)?;

    // Verify we have both events
    let events = json["events"]
        .as_array()
        .expect("events should be an array");
    assert_eq!(
        events.len(),
        2,
        "Both events with identical timestamps should be present"
    );

    // Check both events are there
    let event_types: Vec<&str> = events
        .iter()
        .map(|e| e["event"].as_str().unwrap())
        .collect();
    assert!(event_types.contains(&"policy.decision.allowed:v1"));
    assert!(event_types.contains(&"ritual.completed:v1"));

    // Verify they have the same timestamp
    for event in events {
        assert_eq!(event["ts"].as_str().unwrap(), identical_ts);
    }

    Ok(())
}
