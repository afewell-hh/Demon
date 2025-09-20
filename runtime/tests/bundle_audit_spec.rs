use anyhow::Result;
use runtime::audit::{BundleAuditor, BundleEventType, BundleSource};
use runtime::bundle::{BundleLoader, BundleStatus};
use tempfile::TempDir;

#[tokio::test]
async fn test_audit_event_creation() -> Result<()> {
    // Test bundle loaded event
    let event = BundleAuditor::bundle_loaded(
        "test-tag".to_string(),
        "abc123".to_string(),
        BundleSource::Cache,
        Some("git-sha".to_string()),
        Some("2023-01-01T00:00:00Z".to_string()),
        Some(500),
    );

    assert_eq!(event.event_type, BundleEventType::Loaded);
    assert_eq!(event.tag, "test-tag");
    assert_eq!(event.sha256, Some("abc123".to_string()));
    assert_eq!(event.source, BundleSource::Cache);
    assert_eq!(event.metadata.duration_ms, Some(500));
    assert!(event.metadata.timestamp.starts_with("2"));

    // Test verification failed event
    let event = BundleAuditor::verification_failed(
        "test-tag".to_string(),
        "expected-sha".to_string(),
        "actual-sha".to_string(),
        "remediation advice".to_string(),
    );

    assert_eq!(event.event_type, BundleEventType::VerificationFailed);
    assert_eq!(event.tag, "test-tag");
    assert_eq!(event.sha256, Some("actual-sha".to_string()));
    assert!(event.error.as_ref().unwrap().contains("expected-sha"));
    assert!(event.error.as_ref().unwrap().contains("actual-sha"));

    // Test fallback activated event
    let event = BundleAuditor::fallback_activated(
        "test-tag".to_string(),
        "download failed".to_string(),
        "check network".to_string(),
    );

    assert_eq!(event.event_type, BundleEventType::FallbackActivated);
    assert_eq!(event.source, BundleSource::Fallback);
    assert_eq!(event.error, Some("download failed".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_bundle_state_includes_audit_metadata() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Initial state should be NotLoaded
    let initial_state = loader.state().await;
    assert_eq!(initial_state.status, BundleStatus::NotLoaded);
    assert!(initial_state.metadata.is_none());
    assert!(initial_state.alerts.is_empty());
    assert!(!initial_state.using_fallback);

    // State should include last_check timestamp
    assert!(!initial_state.last_check.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_bundle_download_error_creates_alerts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Try to load a non-existent bundle (should fail and create alerts)
    let result = loader
        .load_bundle(Some("nonexistent-audit-test-tag".to_string()))
        .await;
    assert!(result.is_err());

    // Check that the state reflects the download error
    let state = loader.state().await;
    assert_eq!(state.status, BundleStatus::DownloadError);
    assert!(!state.alerts.is_empty());

    // Check that we have an error alert with remediation
    let error_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| matches!(alert.severity, runtime::bundle::AlertSeverity::Error))
        .collect();
    assert!(!error_alerts.is_empty());

    let error_alert = &error_alerts[0];
    assert!(error_alert.message.contains("Failed to download"));
    assert!(!error_alert.remediation.is_empty());
    assert!(!error_alert.timestamp.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_bundle_verification_failure_creates_alerts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    let bundle_dir = cache_dir.join("verify-fail-test");
    std::fs::create_dir_all(&bundle_dir)?;

    // Create bundle with wrong SHA in manifest
    let bundle = r#"{"schemas": {}, "wit_definitions": {}}"#;
    let manifest = r#"{
        "version": "1.0.0",
        "timestamp": "2023-01-01T00:00:00Z",
        "git": {"sha": "test-sha", "branch": "main"},
        "bundle": "bundle.json",
        "bundle_sha256": "wrong_sha_deliberately_incorrect",
        "description": "Verification failure test"
    }"#;

    std::fs::write(bundle_dir.join("manifest.json"), manifest)?;
    std::fs::write(bundle_dir.join("bundle.json"), bundle)?;

    let loader = BundleLoader::new(Some(cache_dir));
    let result = loader
        .load_bundle(Some("verify-fail-test".to_string()))
        .await;
    assert!(result.is_err());

    // Check that the state has error alerts (may be DownloadError or VerificationFailed depending on path)
    let state = loader.state().await;
    assert!(!state.alerts.is_empty());

    // Check for error alerts with verification-related messages
    let error_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| {
            matches!(alert.severity, runtime::bundle::AlertSeverity::Error)
                && (alert.message.contains("verification failed")
                    || alert.message.contains("SHA-256"))
        })
        .collect();

    // We should have some error alerts, whether from verification failure or download error
    assert!(
        !error_alerts.is_empty(),
        "Expected error alerts but found none"
    );

    Ok(())
}

#[tokio::test]
async fn test_bundle_staleness_detection_creates_alerts() -> Result<()> {
    // Set a very low staleness threshold for testing
    std::env::set_var("DEMON_CONTRACTS_STALE_THRESHOLD_HOURS", "0");

    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    let bundle_dir = cache_dir.join("stale-test-tag");
    std::fs::create_dir_all(&bundle_dir)?;

    // Create a mock manifest with a timestamp from yesterday
    let yesterday = chrono::Utc::now() - chrono::Duration::hours(25);
    let bundle = r#"{"schemas": {}, "wit_definitions": {}}"#;

    // Calculate correct SHA for the bundle content
    let bundle_sha = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bundle.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let manifest = format!(
        r#"{{
        "version": "1.0.0",
        "timestamp": "{}",
        "git": {{
            "sha": "abc123",
            "branch": "main"
        }},
        "bundle": "bundle.json",
        "bundle_sha256": "{}",
        "description": "Stale test bundle"
    }}"#,
        yesterday.to_rfc3339(),
        bundle_sha
    );

    std::fs::write(bundle_dir.join("manifest.json"), manifest)?;
    std::fs::write(bundle_dir.join("bundle.json"), bundle)?;

    let loader = BundleLoader::new(Some(cache_dir));

    // Load the bundle - it should be detected as stale
    let result = loader.load_bundle(Some("stale-test-tag".to_string())).await;
    assert!(result.is_ok()); // Bundle loads successfully

    let state = loader.state().await;
    assert_eq!(state.status, BundleStatus::Stale);

    // Check for staleness warning
    let stale_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| alert.message.contains("stale"))
        .collect();
    assert!(!stale_alerts.is_empty());

    // Clean up
    std::env::remove_var("DEMON_CONTRACTS_STALE_THRESHOLD_HOURS");

    Ok(())
}

#[tokio::test]
async fn test_bundle_fallback_creates_alerts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Try to load with fallback (should fail and set using_fallback state)
    let result = loader
        .load_with_fallback(Some("nonexistent-fallback-test".to_string()))
        .await;
    assert!(result.is_err()); // Will fail since embedded schemas aren't implemented

    // Check state shows fallback attempt
    let state = loader.state().await;
    assert_eq!(state.status, BundleStatus::UsingFallback);
    assert!(state.using_fallback);

    // Check for fallback alert
    let fallback_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| alert.message.contains("fallback"))
        .collect();
    assert!(!fallback_alerts.is_empty());

    let fallback_alert = &fallback_alerts[0];
    assert!(!fallback_alert.remediation.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_successful_bundle_load_updates_state() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    let bundle_dir = cache_dir.join("success-test-tag");
    std::fs::create_dir_all(&bundle_dir)?;

    // Create valid bundle and manifest
    let bundle = r#"{
        "schemas": {
            "test.json": "{\"type\":\"object\"}"
        },
        "wit_definitions": {}
    }"#;

    let bundle_sha = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bundle.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let manifest = format!(
        r#"{{
        "version": "1.0.0",
        "timestamp": "{}",
        "git": {{
            "sha": "test-git-sha",
            "branch": "main"
        }},
        "bundle": "bundle.json",
        "bundle_sha256": "{}",
        "description": "Test bundle for success case"
    }}"#,
        chrono::Utc::now().to_rfc3339(),
        bundle_sha
    );

    std::fs::write(bundle_dir.join("manifest.json"), manifest)?;
    std::fs::write(bundle_dir.join("bundle.json"), bundle)?;

    let loader = BundleLoader::new(Some(cache_dir));

    // Load bundle from cache
    let result = loader
        .load_bundle(Some("success-test-tag".to_string()))
        .await;
    assert!(result.is_ok());

    // Check state shows successful load
    let state = loader.state().await;
    assert_eq!(state.status, BundleStatus::Loaded);
    assert!(!state.using_fallback);

    // Should have metadata
    assert!(state.metadata.is_some());
    let metadata = state.metadata.unwrap();
    assert_eq!(metadata.tag, "success-test-tag");
    assert_eq!(metadata.sha256, bundle_sha);
    assert_eq!(metadata.git_sha, "test-git-sha");

    Ok(())
}

#[tokio::test]
async fn test_metrics_integration_does_not_panic() -> Result<()> {
    // This test verifies that metrics calls don't panic
    // In a real environment, you would use a metrics recorder to capture values

    // Create some audit events and emit them - should not panic
    let event = BundleAuditor::bundle_loaded(
        "metrics-test".to_string(),
        "sha123".to_string(),
        BundleSource::Download,
        Some("git-sha".to_string()),
        Some("2023-01-01T00:00:00Z".to_string()),
        Some(1500),
    );
    BundleAuditor::emit_event(event);

    let event = BundleAuditor::verification_failed(
        "metrics-test".to_string(),
        "expected".to_string(),
        "actual".to_string(),
        "remediation".to_string(),
    );
    BundleAuditor::emit_event(event);

    let event = BundleAuditor::stale_detected(
        "metrics-test".to_string(),
        "2023-01-01T00:00:00Z".to_string(),
        48,
        "update bundle".to_string(),
    );
    BundleAuditor::emit_event(event);

    Ok(())
}

#[test]
fn test_audit_event_serialization() -> Result<()> {
    let event = BundleAuditor::bundle_loaded(
        "test-tag".to_string(),
        "abc123".to_string(),
        BundleSource::Cache,
        Some("git-sha".to_string()),
        Some("2023-01-01T00:00:00Z".to_string()),
        Some(500),
    );

    // Should be serializable to JSON
    let json = serde_json::to_string(&event)?;
    assert!(json.contains("\"event_type\":\"Loaded\""));
    assert!(json.contains("\"tag\":\"test-tag\""));
    assert!(json.contains("\"sha256\":\"abc123\""));

    // Should be deserializable from JSON
    let deserialized: runtime::audit::BundleAuditEvent = serde_json::from_str(&json)?;
    assert_eq!(deserialized.tag, "test-tag");
    assert_eq!(deserialized.sha256, Some("abc123".to_string()));

    Ok(())
}

#[test]
fn test_all_audit_event_types_creation() -> Result<()> {
    // Test all event types can be created without panicking

    let _loaded = BundleAuditor::bundle_loaded(
        "test".to_string(),
        "sha".to_string(),
        BundleSource::Cache,
        Some("git".to_string()),
        Some("time".to_string()),
        Some(100),
    );

    let _verification_failed = BundleAuditor::verification_failed(
        "test".to_string(),
        "expected".to_string(),
        "actual".to_string(),
        "remediation".to_string(),
    );

    let _fallback = BundleAuditor::fallback_activated(
        "test".to_string(),
        "error".to_string(),
        "remediation".to_string(),
    );

    let _download_failed = BundleAuditor::download_failed(
        "test".to_string(),
        "error".to_string(),
        Some("network_error".to_string()),
        "remediation".to_string(),
    );

    let _stale = BundleAuditor::stale_detected(
        "test".to_string(),
        "timestamp".to_string(),
        24,
        "remediation".to_string(),
    );

    let _update = BundleAuditor::update_detected("test".to_string(), "remediation".to_string());

    let _refresh = BundleAuditor::refresh_attempt("test".to_string());

    let _status = BundleAuditor::status_check("test".to_string());

    Ok(())
}
