use axum::http::StatusCode;
use axum_test::TestServer;
use operate_ui::{create_app, AppState};

async fn setup_test_server() -> TestServer {
    let state = AppState::new().await;
    let app = create_app(state);
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn test_bundle_status_api_returns_contract_bundle_info() {
    let server = setup_test_server().await;

    let response = server.get("/api/contracts/status").await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();

    // Should have contractBundle field
    assert!(body["contractBundle"].is_object());

    // Should have status field
    assert!(body["contractBundle"]["status"].is_string());
    let status = body["contractBundle"]["status"].as_str().unwrap();
    assert!(matches!(
        status,
        "loaded"
            | "not_loaded"
            | "disabled"
            | "stale"
            | "using_fallback"
            | "verification_failed"
            | "download_error"
    ));

    // Should have alerts field
    assert!(body["contractBundle"]["alerts"].is_array());

    // If disabled, metadata should be null
    if status == "disabled" {
        assert!(body["contractBundle"]["metadata"].is_null());
        assert_eq!(
            body["contractBundle"]["source"].as_str().unwrap(),
            "disabled"
        );
    }

    // If loaded or not_loaded, should have metadata and cacheDir
    if status != "disabled" {
        assert!(body["contractBundle"]["cacheDir"].is_string());
    }
}

#[tokio::test]
async fn test_bundle_status_api_handles_disabled_bundles() {
    // Set environment variable to disable bundle loading
    std::env::set_var("DEMON_SKIP_CONTRACT_BUNDLE", "1");

    let server = setup_test_server().await;

    let response = server.get("/api/contracts/status").await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();

    assert_eq!(
        body["contractBundle"]["status"].as_str().unwrap(),
        "disabled"
    );
    assert!(body["contractBundle"]["metadata"].is_null());
    assert_eq!(
        body["contractBundle"]["source"].as_str().unwrap(),
        "disabled"
    );

    // Clean up
    std::env::remove_var("DEMON_SKIP_CONTRACT_BUNDLE");
}

#[tokio::test]
async fn test_bundle_status_api_content_type() {
    let server = setup_test_server().await;

    let response = server.get("/api/contracts/status").await;

    response.assert_status(StatusCode::OK);
    // Check content-type header is JSON
    let headers = response.headers();
    let content_type = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn test_bundle_status_api_includes_alert_fields() {
    let server = setup_test_server().await;

    let response = server.get("/api/contracts/status").await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();

    // Check alert structure if alerts are present
    if let Some(alerts) = body["contractBundle"]["alerts"].as_array() {
        for alert in alerts {
            // Each alert should have required fields
            assert!(alert["severity"].is_string());
            assert!(alert["message"].is_string());
            assert!(alert["remediation"].is_string());
            assert!(alert["timestamp"].is_string());

            // Severity should be a valid value
            let severity = alert["severity"].as_str().unwrap();
            assert!(matches!(severity, "error" | "warning" | "info"));
        }
    }

    // Should have new fields
    assert!(body["contractBundle"]["lastCheck"].is_string());
    assert!(body["contractBundle"]["usingFallback"].is_boolean());
}
