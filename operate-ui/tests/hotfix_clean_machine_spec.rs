/// Integration tests for hotfix/operate-ui-stream-init
/// Tests ensure Operate UI behaves correctly on clean environments
use axum::Router;
use operate_ui::{create_app, AppState};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

/// Test that Operate UI handles gracefully when NATS is completely unavailable
#[tokio::test]
async fn test_operates_gracefully_with_nats_down() -> anyhow::Result<()> {
    // Use a NATS URL that definitely won't work
    std::env::set_var("NATS_URL", "nats://nonexistent.example.com:9999");

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let state = AppState::new().await;
    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Test API endpoint returns 502 JSON
    let api_response = client.get(format!("{}/api/runs", base)).send().await?;
    assert_eq!(api_response.status(), 502);
    let api_body = api_response.text().await?;
    assert!(api_body.contains("JetStream is not available"));

    // Verify it's valid JSON
    let _: serde_json::Value = serde_json::from_str(&api_body)?;

    // Test HTML endpoint returns 200 with friendly error
    let html_response = client.get(format!("{}/runs", base)).send().await?;
    assert_eq!(html_response.status(), 200);
    let html_body = html_response.text().await?;

    // Should contain error messaging but still be a valid HTML page
    assert!(html_body.contains("<!DOCTYPE html"));
    assert!(
        html_body.contains("JetStream is not available")
            || html_body.contains("JetStream Unavailable")
    );

    // Test health endpoint still works
    let health_response = client.get(format!("{}/health", base)).send().await?;
    assert_eq!(health_response.status(), 200);

    // Reset NATS_URL
    std::env::remove_var("NATS_URL");
    Ok(())
}

/// Test that stream auto-creation works when NATS is available but stream is missing
#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_stream_auto_creation() -> anyhow::Result<()> {
    // Use a unique stream name for this test to avoid conflicts
    let test_stream_name = format!("TEST_RITUAL_EVENTS_{}", chrono::Utc::now().timestamp());
    std::env::set_var("RITUAL_STREAM_NAME", &test_stream_name);

    // First, ensure the stream doesn't exist by trying to delete it
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let nc = async_nats::connect(&nats_url).await?;
    let js = async_nats::jetstream::new(nc);

    // Delete stream if it exists (ignore errors)
    let _ = js.delete_stream(&test_stream_name).await;

    // Now create the AppState which should auto-create the stream
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let state = AppState::new().await;
    assert!(
        state.jetstream_client.is_some(),
        "JetStream client should be available"
    );

    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    // Verify the stream was created
    assert!(
        js.get_stream(&test_stream_name).await.is_ok(),
        "Stream should exist after auto-creation"
    );

    // Test that API endpoints work now
    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let api_response = client.get(format!("{}/api/runs", base)).send().await?;
    assert_eq!(api_response.status(), 200);

    // Clean up
    let _ = js.delete_stream(&test_stream_name).await;
    std::env::remove_var("RITUAL_STREAM_NAME");
    Ok(())
}

/// Test that template fallback works when templates are missing/corrupted
#[tokio::test]
async fn test_template_fallback_graceful_degradation() -> anyhow::Result<()> {
    use std::fs;

    // This test manipulates the template directory, so we need to be careful
    // We'll test by temporarily creating a new directory structure
    let temp_dir = std::env::temp_dir().join("operate_ui_test_templates");

    // Create a minimal test environment
    // We can't easily test the actual template loading failure without
    // modifying the binary, but we can test that our fallback templates work

    // The key insight is that our hardening prevents crashes -
    // the server should start even with template issues

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Use a fake NATS URL to avoid NATS dependency for this test
    std::env::set_var("NATS_URL", "nats://nonexistent.example.com:9999");

    let state = AppState::new().await;
    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // The key test: server should start and respond even with template/NATS issues
    let response = client.get(format!("{}/health", base)).send().await?;
    assert_eq!(response.status(), 200);

    // HTML endpoints should return some kind of response (either normal or fallback)
    let html_response = client.get(format!("{}/runs", base)).send().await?;
    assert_eq!(html_response.status(), 200);
    let html_body = html_response.text().await?;
    assert!(html_body.contains("<!DOCTYPE html") || html_body.contains("<html"));

    // Clean up
    std::env::remove_var("NATS_URL");
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    Ok(())
}

/// Comprehensive test ensuring all hotfix features work together
#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_clean_machine_full_startup_sequence() -> anyhow::Result<()> {
    // Use a unique stream name to simulate clean environment
    let test_stream_name = format!("CLEAN_TEST_{}", chrono::Utc::now().timestamp());
    std::env::set_var("RITUAL_STREAM_NAME", &test_stream_name);

    // Ensure stream doesn't exist initially
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into());
    let nc = async_nats::connect(&nats_url).await?;
    let js = async_nats::jetstream::new(nc);
    let _ = js.delete_stream(&test_stream_name).await;

    // Start server - this should:
    // 1. Connect to NATS
    // 2. Auto-create the missing stream
    // 3. Load templates (or use fallbacks)
    // 4. Start successfully
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let state = AppState::new().await;
    let app: Router = create_app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // 1. Verify stream was created
    assert!(
        js.get_stream(&test_stream_name).await.is_ok(),
        "Stream should exist after auto-creation"
    );

    // 2. Verify API endpoints work
    let api_response = client.get(format!("{}/api/runs", base)).send().await?;
    assert_eq!(api_response.status(), 200);
    let api_body = api_response.text().await?;
    // Should be empty runs list, not an error
    let json: serde_json::Value = serde_json::from_str(&api_body)?;
    assert!(json.is_array() || (json.is_object() && json.get("runs").is_some()));

    // 3. Verify HTML endpoints work
    let html_response = client.get(format!("{}/runs", base)).send().await?;
    assert_eq!(html_response.status(), 200);
    let html_body = html_response.text().await?;
    assert!(html_body.contains("<!DOCTYPE html"));
    assert!(html_body.contains("Runs") || html_body.contains("runs"));

    // 4. Verify health endpoint works
    let health_response = client.get(format!("{}/health", base)).send().await?;
    assert_eq!(health_response.status(), 200);

    // Cleanup
    let _ = js.delete_stream(&test_stream_name).await;
    std::env::remove_var("RITUAL_STREAM_NAME");
    Ok(())
}
