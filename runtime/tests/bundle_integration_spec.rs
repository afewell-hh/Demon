use anyhow::Result;
use runtime::bundle::{BundleLoader, ContractBundle};
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test]
async fn test_bundle_loader_cache_directory_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("contracts");

    let loader = BundleLoader::new(Some(cache_dir.clone()));

    // Verify cache directory is set correctly
    assert_eq!(loader.cache_dir(), &cache_dir);

    // Verify no metadata initially
    assert!(loader.metadata().await.is_none());

    Ok(())
}

#[tokio::test]
async fn test_bundle_extraction_to_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let extract_dir = temp_dir.path().join("extracted");

    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Create a mock bundle
    let mut schemas = HashMap::new();
    schemas.insert(
        "echo-config.v1.json".to_string(),
        r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "messagePrefix": { "type": "string", "default": "" }
        }
    }"#
        .to_string(),
    );

    schemas.insert(
        "result-envelope.v1.json".to_string(),
        r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "result": { "type": "object" }
        }
    }"#
        .to_string(),
    );

    let mut wit_definitions = HashMap::new();
    wit_definitions.insert(
        "echo.wit".to_string(),
        r#"
        package echo:types;

        interface echo {
            record message {
                content: string,
            }
        }
    "#
        .to_string(),
    );

    let bundle = ContractBundle {
        schemas,
        wit_definitions,
    };

    // Extract bundle to directory
    loader.extract_to_dir(&bundle, &extract_dir).await?;

    // Verify extracted files
    assert!(extract_dir.join("schemas").is_dir());
    assert!(extract_dir.join("config").is_dir());
    assert!(extract_dir.join("wit").is_dir());

    // Verify config schema was placed correctly
    let config_file = extract_dir.join("config").join("echo-config.v1.json");
    assert!(config_file.exists());
    let config_content = std::fs::read_to_string(&config_file)?;
    assert!(config_content.contains("messagePrefix"));

    // Verify result schema was placed correctly
    let result_file = extract_dir.join("schemas").join("result-envelope.v1.json");
    assert!(result_file.exists());
    let result_content = std::fs::read_to_string(&result_file)?;
    assert!(result_content.contains("result"));

    // Verify WIT file was placed correctly
    let wit_file = extract_dir.join("wit").join("echo.wit");
    assert!(wit_file.exists());
    let wit_content = std::fs::read_to_string(&wit_file)?;
    assert!(wit_content.contains("package echo:types"));

    Ok(())
}

#[tokio::test]
async fn test_bundle_loader_environment_configuration() -> Result<()> {
    // Test default cache directory
    let loader = BundleLoader::new(None);
    assert!(loader
        .cache_dir()
        .to_string_lossy()
        .contains(".demon/contracts"));

    // Test custom cache directory
    let temp_dir = TempDir::new()?;
    let custom_cache = temp_dir.path().join("custom_cache");
    let loader = BundleLoader::new(Some(custom_cache.clone()));
    assert_eq!(loader.cache_dir(), &custom_cache);

    Ok(())
}

#[tokio::test]
async fn test_bundle_load_with_skip_verification() -> Result<()> {
    // Set environment variable to skip verification
    std::env::set_var("DEMON_SKIP_BUNDLE_VERIFICATION", "1");

    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // This should fail gracefully when trying to download from GitHub
    // but not due to verification issues
    let result = loader
        .load_bundle(Some("nonexistent-tag".to_string()))
        .await;
    assert!(result.is_err());

    // Clean up
    std::env::remove_var("DEMON_SKIP_BUNDLE_VERIFICATION");

    Ok(())
}

#[test]
fn test_bundle_loader_with_fallback() {
    // This would test the fallback to embedded schemas
    // For now, we just verify the fallback method exists and returns an error
    // since embedded schemas are not implemented yet
    let rt = tokio::runtime::Runtime::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    rt.block_on(async {
        let result = loader
            .load_with_fallback(Some("nonexistent-tag".to_string()))
            .await;
        // Should fail since both download and embedded fallback fail
        assert!(result.is_err());
    });
}

// Tests for bundle drift detection and alerting

#[tokio::test]
async fn test_bundle_state_tracking() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Initial state should be NotLoaded
    let initial_state = loader.state().await;
    assert_eq!(
        initial_state.status,
        runtime::bundle::BundleStatus::NotLoaded
    );
    assert!(initial_state.metadata.is_none());
    assert!(initial_state.alerts.is_empty());
    assert!(!initial_state.using_fallback);

    Ok(())
}

#[tokio::test]
async fn test_bundle_download_error_alerts() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Try to load a non-existent bundle
    let result = loader
        .load_bundle(Some("nonexistent-tag-404".to_string()))
        .await;
    assert!(result.is_err());

    // Check that the state reflects the download error
    let state = loader.state().await;
    assert_eq!(state.status, runtime::bundle::BundleStatus::DownloadError);
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

    Ok(())
}

#[tokio::test]
async fn test_bundle_verification_disabled_warning() -> Result<()> {
    // Set up environment to skip verification
    std::env::set_var("DEMON_SKIP_BUNDLE_VERIFICATION", "1");

    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    let bundle_dir = cache_dir.join("test-tag");
    std::fs::create_dir_all(&bundle_dir)?;

    // Create a mock manifest and bundle with mismatched SHA (this would normally fail verification)
    let manifest = r#"{
        "version": "1.0.0",
        "timestamp": "2023-01-01T00:00:00Z",
        "git": {
            "sha": "abc123",
            "branch": "main"
        },
        "bundle": "bundle.json",
        "bundle_sha256": "wrong_sha_this_would_normally_fail",
        "description": "Test bundle"
    }"#;

    let bundle = r#"{
        "schemas": {
            "test.json": "{\"type\":\"object\"}"
        },
        "wit_definitions": {}
    }"#;

    std::fs::write(bundle_dir.join("manifest.json"), manifest)?;
    std::fs::write(bundle_dir.join("bundle.json"), bundle)?;

    let loader = BundleLoader::new(Some(cache_dir));

    // This should succeed but with a warning alert
    let result = loader.load_bundle(Some("test-tag".to_string())).await;

    // Check the alerts contain a verification disabled warning
    let state = loader.state().await;
    let warning_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| matches!(alert.severity, runtime::bundle::AlertSeverity::Warning))
        .collect();

    assert!(!warning_alerts.is_empty());
    let verification_warning = warning_alerts
        .iter()
        .find(|alert| alert.message.contains("verification is disabled"));
    assert!(verification_warning.is_some());

    // Clean up
    std::env::remove_var("DEMON_SKIP_BUNDLE_VERIFICATION");

    // If bundle loading succeeded despite wrong SHA, verification was indeed skipped
    if result.is_ok() {
        println!("Verification skip test passed - bundle loaded despite wrong SHA");
    }

    Ok(())
}

#[tokio::test]
async fn test_bundle_staleness_detection() -> Result<()> {
    // Set a very low staleness threshold for testing
    std::env::set_var("DEMON_CONTRACTS_STALE_THRESHOLD_HOURS", "0");

    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    let bundle_dir = cache_dir.join("stale-test-tag");
    std::fs::create_dir_all(&bundle_dir)?;

    // Create a mock manifest with a timestamp from yesterday
    let yesterday = chrono::Utc::now() - chrono::Duration::hours(25);
    let _manifest = format!(
        r#"{{
        "version": "1.0.0",
        "timestamp": "{}",
        "git": {{
            "sha": "abc123",
            "branch": "main"
        }},
        "bundle": "bundle.json",
        "bundle_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "description": "Stale test bundle"
    }}"#,
        yesterday.to_rfc3339()
    );

    let bundle = r#"{
        "schemas": {
            "test.json": "{\"type\":\"object\"}"
        },
        "wit_definitions": {}
    }"#;

    // Calculate correct SHA for the bundle content
    let bundle_sha = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bundle.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    // Update manifest with correct SHA
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
    assert_eq!(state.status, runtime::bundle::BundleStatus::Stale);

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
async fn test_bundle_fallback_alert_tracking() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Try to load with fallback (should fail and set using_fallback state)
    let result = loader
        .load_with_fallback(Some("nonexistent-tag".to_string()))
        .await;
    assert!(result.is_err()); // Will fail since embedded schemas aren't implemented

    // Check state shows fallback attempt
    let state = loader.state().await;
    assert_eq!(state.status, runtime::bundle::BundleStatus::UsingFallback);
    assert!(state.using_fallback);

    // Check for fallback alert
    let fallback_alerts: Vec<_> = state
        .alerts
        .iter()
        .filter(|alert| alert.message.contains("fallback"))
        .collect();
    assert!(!fallback_alerts.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_network_remediation_context_awareness() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let loader = BundleLoader::new(Some(temp_dir.path().join("cache")));

    // Temporarily unset GH_TOKEN to test remediation advice
    let original_token = std::env::var("GH_TOKEN").ok();
    std::env::remove_var("GH_TOKEN");

    // Try to load a bundle (will fail)
    let _result = loader
        .load_bundle(Some("nonexistent-tag".to_string()))
        .await;

    let state = loader.state().await;

    // Check that remediation mentions GH_TOKEN
    let has_token_advice = state
        .alerts
        .iter()
        .any(|alert| alert.remediation.contains("GH_TOKEN"));
    assert!(has_token_advice);

    // Restore original token if it existed
    if let Some(token) = original_token {
        std::env::set_var("GH_TOKEN", token);
    }

    Ok(())
}
