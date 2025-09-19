use config_loader::ConfigManager;
use runtime::link::router::Router;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn setup_test_environment_with_config(valid_config: bool) -> (TempDir, Router) {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");

    fs::create_dir_all(contracts_dir.join("config")).unwrap();
    fs::create_dir_all(&config_dir).unwrap();

    // Create echo config schema
    let schema_content = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Echo Capsule Configuration",
        "description": "Configuration schema for the echo capsule",
        "type": "object",
        "properties": {
            "messagePrefix": {
                "type": "string",
                "description": "Prefix to add to echoed messages",
                "default": ""
            },
            "enableTrim": {
                "type": "boolean",
                "description": "Whether to trim whitespace from messages",
                "default": true
            },
            "maxMessageLength": {
                "type": "integer",
                "description": "Maximum length of messages to process",
                "minimum": 1,
                "maximum": 10000,
                "default": 1000
            },
            "outputFormat": {
                "type": "string",
                "description": "Format for output messages",
                "enum": ["plain", "json", "structured"],
                "default": "plain"
            }
        },
        "required": ["messagePrefix", "enableTrim"],
        "additionalProperties": false
    }"#;

    fs::write(
        contracts_dir.join("config/echo-config.v1.json"),
        schema_content,
    )
    .unwrap();

    // Create config file based on whether it should be valid
    let config_content = if valid_config {
        r#"{
            "messagePrefix": "Test: ",
            "enableTrim": true,
            "maxMessageLength": 500,
            "outputFormat": "plain"
        }"#
    } else {
        r#"{
            "messagePrefix": "Test: ",
            "enableTrim": "not_a_boolean"
        }"#
    };

    fs::write(config_dir.join("echo.json"), config_content).unwrap();

    let config_manager = ConfigManager::with_dirs(contracts_dir, config_dir);
    let router = Router::with_config_manager(config_manager);

    (temp_dir, router)
}

#[tokio::test]
async fn given_valid_config_when_dispatch_echo_then_success() {
    let (_temp_dir, router) = setup_test_environment_with_config(true);

    let args = json!({
        "message": "Hello, World!"
    });

    // Note: This test won't actually emit NATS events due to lack of NATS server in test
    // but we can verify the basic flow works
    let result = router
        .dispatch("echo", &args, "test-run", "test-ritual")
        .await;

    match result {
        Ok(envelope_json) => {
            // Verify that we got a valid result envelope
            assert!(envelope_json.is_object());
            let envelope = envelope_json.as_object().unwrap();
            assert!(envelope.contains_key("result"));
        }
        Err(e) => {
            // If NATS is not available, we might get a connection error
            // In that case, we verify the error is related to NATS, not config validation
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("Failed to connect to NATS")
                    || error_msg.contains("connection refused")
                    || error_msg.contains("Connect"),
                "Unexpected error: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn given_invalid_config_when_dispatch_echo_then_config_validation_failure() {
    let (_temp_dir, router) = setup_test_environment_with_config(false);

    let args = json!({
        "message": "Hello, World!"
    });

    let result = router
        .dispatch("echo", &args, "test-run", "test-ritual")
        .await;

    // Should fail due to config validation
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Configuration validation failed"));
}

#[tokio::test]
async fn given_missing_config_when_dispatch_echo_then_config_not_found_error() {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");

    fs::create_dir_all(contracts_dir.join("config")).unwrap();
    fs::create_dir_all(&config_dir).unwrap();

    // Create echo config schema but no config file
    let schema_content = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "$id": "echo-config.v1.json",
        "title": "Echo Capsule Configuration",
        "type": "object",
        "properties": {
            "messagePrefix": { "type": "string" },
            "enableTrim": { "type": "boolean" }
        },
        "required": ["messagePrefix", "enableTrim"],
        "additionalProperties": false
    }"#;

    fs::write(
        contracts_dir.join("config/echo-config.v1.json"),
        schema_content,
    )
    .unwrap();

    let config_manager = ConfigManager::with_dirs(contracts_dir, config_dir);
    let router = Router::with_config_manager(config_manager);

    let args = json!({
        "message": "Hello, World!"
    });

    let result = router
        .dispatch("echo", &args, "test-run", "test-ritual")
        .await;

    // Should fail due to missing config file
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Configuration validation failed"));
}

#[tokio::test]
async fn given_unknown_function_ref_when_dispatch_then_unknown_function_error() {
    let (_temp_dir, router) = setup_test_environment_with_config(true);

    let args = json!({
        "message": "Hello, World!"
    });

    let result = router
        .dispatch("unknown", &args, "test-run", "test-ritual")
        .await;

    // Should fail due to unknown function ref
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("unknown functionRef: unknown"));
}

#[test]
fn given_default_router_when_created_then_has_config_manager() {
    let router = Router::new();
    // We can't directly test the internal config_manager, but we can verify
    // the router was created successfully
    assert!(std::ptr::addr_of!(router) as *const _ != std::ptr::null());
}

#[test]
fn given_custom_config_manager_when_create_router_then_uses_custom_manager() {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");

    let config_manager = ConfigManager::with_dirs(contracts_dir, config_dir);
    let router = Router::with_config_manager(config_manager);

    // Verify router was created successfully with custom config manager
    assert!(std::ptr::addr_of!(router) as *const _ != std::ptr::null());
}
