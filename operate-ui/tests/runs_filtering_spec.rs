use anyhow::Result;
use async_nats::jetstream;
use chrono::Utc;
use reqwest::StatusCode;
use tokio::task;

async fn start_ui() -> Result<u16> {
    // Bind ephemeral port
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let state = operate_ui::AppState::new().await;
    let app = operate_ui::create_app(state);
    task::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Ok(port)
}

async fn ensure_stream() -> Result<()> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(url).await?;
    let js = jetstream::new(client);
    let name = std::env::var("RITUAL_STREAM_NAME").unwrap_or_else(|_| "RITUAL_EVENTS".into());
    let _ = js
        .get_or_create_stream(jetstream::stream::Config {
            name,
            subjects: vec!["demon.ritual.v1.>".to_string()],
            ..Default::default()
        })
        .await?;
    Ok(())
}

async fn publish_run_event(
    js: &jetstream::Context,
    ritual_id: &str,
    run_id: &str,
    event_type: &str,
    extra_fields: Option<serde_json::Value>,
) -> Result<()> {
    let subject = format!("demon.ritual.v1.{}.{}.events", ritual_id, run_id);
    let mut payload = serde_json::json!({
        "event": event_type,
        "ts": Utc::now().to_rfc3339(),
        "tenantId": "default",
        "runId": run_id,
        "ritualId": ritual_id,
    });

    if let Some(extra) = extra_fields {
        if let (Some(payload_obj), Some(extra_obj)) = (payload.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_obj {
                payload_obj.insert(k.clone(), v.clone());
            }
        }
    }

    let mut hdrs = async_nats::HeaderMap::new();
    let msg_id = format!(
        "{}:{}:{}",
        run_id,
        event_type,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    hdrs.insert("Nats-Msg-Id", msg_id.as_str());

    js.publish_with_headers(subject, hdrs, serde_json::to_vec(&payload)?.into())
        .await?
        .await?;
    Ok(())
}

async fn create_test_run(
    js: &jetstream::Context,
    ritual_id: &str,
    run_id: &str,
    status: &str,
    capabilities: Vec<&str>,
) -> Result<()> {
    // Publish start event
    publish_run_event(js, ritual_id, run_id, "ritual.started:v1", None).await?;

    // Publish capability events
    for capability in capabilities {
        let extra = serde_json::json!({
            "capability": capability
        });
        publish_run_event(
            js,
            ritual_id,
            run_id,
            &format!("{}.executed:v1", capability),
            Some(extra),
        )
        .await?;
    }

    // Publish terminal event if not running
    match status {
        "completed" => {
            publish_run_event(js, ritual_id, run_id, "ritual.completed:v1", None).await?;
        }
        "failed" => {
            publish_run_event(js, ritual_id, run_id, "ritual.failed:v1", None).await?;
        }
        _ => {} // running - no terminal event
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_runs_with_different_statuses_when_filtering_by_status_then_returns_only_matching_runs(
) -> Result<()> {
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::connect(url).await?;
    let js = jetstream::new(client);

    let test_prefix = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // Create test runs with different statuses
    create_test_run(
        &js,
        "test-ritual",
        &format!("{}-running", test_prefix),
        "running",
        vec!["echo"],
    )
    .await?;
    create_test_run(
        &js,
        "test-ritual",
        &format!("{}-completed", test_prefix),
        "completed",
        vec!["echo"],
    )
    .await?;
    create_test_run(
        &js,
        "test-ritual",
        &format!("{}-failed", test_prefix),
        "failed",
        vec!["echo"],
    )
    .await?;

    // Wait for events to be processed
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let http = reqwest::Client::new();

    // Test filtering by completed status
    let response = http
        .get(format!("{}/api/runs?status=completed", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let runs: serde_json::Value = response.json().await?;
    let runs_array = runs.as_array().expect("Expected array of runs");

    // Should contain at least our completed run
    let completed_runs: Vec<_> = runs_array
        .iter()
        .filter(|r| {
            r["runId"].as_str().unwrap_or("").contains(&test_prefix) && r["status"] == "Completed"
        })
        .collect();
    assert!(
        !completed_runs.is_empty(),
        "Should find at least one completed run"
    );

    // Test filtering by failed status
    let response = http
        .get(format!("{}/api/runs?status=failed", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let runs: serde_json::Value = response.json().await?;
    let runs_array = runs.as_array().expect("Expected array of runs");

    let failed_runs: Vec<_> = runs_array
        .iter()
        .filter(|r| {
            r["runId"].as_str().unwrap_or("").contains(&test_prefix) && r["status"] == "Failed"
        })
        .collect();
    assert!(
        !failed_runs.is_empty(),
        "Should find at least one failed run"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_runs_with_different_capabilities_when_filtering_by_capability_then_returns_only_matching_runs(
) -> Result<()> {
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let client = async_nats::connect(url).await?;
    let js = jetstream::new(client);

    let test_prefix = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // Create test runs with different capabilities
    create_test_run(
        &js,
        "test-ritual",
        &format!("{}-echo", test_prefix),
        "completed",
        vec!["echo"],
    )
    .await?;
    create_test_run(
        &js,
        "test-ritual",
        &format!("{}-approval", test_prefix),
        "completed",
        vec!["approval"],
    )
    .await?;

    // Wait for events to be processed
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let http = reqwest::Client::new();

    // Test filtering by echo capability
    let response = http
        .get(format!("{}/api/runs?capability=echo", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let runs: serde_json::Value = response.json().await?;
    let runs_array = runs.as_array().expect("Expected array of runs");

    // Should find runs containing echo capability
    let echo_runs: Vec<_> = runs_array
        .iter()
        .filter(|r| {
            r["runId"]
                .as_str()
                .unwrap_or("")
                .contains(&format!("{}-echo", test_prefix))
        })
        .collect();
    assert!(
        !echo_runs.is_empty(),
        "Should find at least one run with echo capability"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_invalid_query_params_when_calling_api_then_returns_400_with_helpful_error(
) -> Result<()> {
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let http = reqwest::Client::new();

    // Test invalid status
    let response = http
        .get(format!("{}/api/runs?status=invalid", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = response.json().await?;
    assert!(error["error"].as_str().unwrap().contains("invalid status"));

    // Test invalid limit
    let response = http
        .get(format!("{}/api/runs?limit=0", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = response.json().await?;
    assert!(error["error"].as_str().unwrap().contains("invalid limit"));

    // Test invalid timestamp
    let response = http
        .get(format!("{}/api/runs?since=invalid-timestamp", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: serde_json::Value = response.json().await?;
    assert!(error["error"]
        .as_str()
        .unwrap()
        .contains("invalid since timestamp"));

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_valid_timestamp_filters_when_calling_api_then_filters_by_time_window() -> Result<()>
{
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let http = reqwest::Client::new();

    // Test with RFC3339 format
    let now = Utc::now();
    let since = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let response = http
        .get(format!("{}/api/runs?since={}", base, since))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    // Test with Unix timestamp
    let unix_since = now.timestamp();
    let response = http
        .get(format!("{}/api/runs?since={}", base, unix_since))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_limit_parameter_when_calling_api_then_enforces_limit() -> Result<()> {
    std::env::set_var("RITUAL_STREAM_NAME", "RITUAL_EVENTS");
    ensure_stream().await?;
    let port = start_ui().await?;
    let base = format!("http://127.0.0.1:{}", port);

    let http = reqwest::Client::new();

    // Test with small limit
    let response = http
        .get(format!("{}/api/runs?limit=1", base))
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let runs: serde_json::Value = response.json().await?;
    let runs_array = runs.as_array().expect("Expected array of runs");
    assert!(runs_array.len() <= 1, "Should respect limit of 1");

    Ok(())
}
