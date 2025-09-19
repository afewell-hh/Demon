use config_loader::{ConfigError, ConfigManager};
use serde::Deserialize;
use std::fs;
use tempfile::TempDir;

#[derive(Deserialize, Debug, PartialEq)]
struct EchoConfig {
    #[serde(rename = "messagePrefix")]
    message_prefix: String,
    #[serde(rename = "enableTrim")]
    enable_trim: bool,
    #[serde(rename = "maxMessageLength")]
    max_message_length: Option<i32>,
    #[serde(rename = "outputFormat")]
    output_format: Option<String>,
}

fn setup_test_environment() -> (TempDir, ConfigManager) {
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

    let manager = ConfigManager::with_dirs(contracts_dir, config_dir);
    (temp_dir, manager)
}

#[test]
fn given_valid_config_when_load_then_success() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    fs::write(manager.config_dir().join("echo.json"), config_content).unwrap();

    let config: EchoConfig = manager.load("echo").unwrap();
    assert_eq!(config.message_prefix, "Test: ");
    assert!(config.enable_trim);
    assert_eq!(config.max_message_length, Some(500));
    assert_eq!(config.output_format, Some("plain".to_string()));
}

#[test]
fn given_config_missing_required_field_when_load_then_validation_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "messagePrefix": "Test: "
    }"#;

    fs::write(manager.config_dir().join("echo.json"), config_content).unwrap();

    let result: Result<EchoConfig, ConfigError> = manager.load("echo");
    assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));

    if let Err(ConfigError::ValidationFailed { errors }) = result {
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| e.json_pointer.contains("enableTrim") || e.message.contains("enableTrim")));
    }
}

#[test]
fn given_config_with_invalid_types_when_load_then_validation_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": "not_a_boolean",
        "maxMessageLength": "not_a_number",
        "outputFormat": "invalid_format"
    }"#;

    fs::write(manager.config_dir().join("echo.json"), config_content).unwrap();

    let result: Result<EchoConfig, ConfigError> = manager.load("echo");
    assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));

    if let Err(ConfigError::ValidationFailed { errors }) = result {
        assert!(errors.len() >= 3); // At least 3 validation errors
    }
}

#[test]
fn given_nonexistent_config_file_when_load_then_file_not_found_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let result: Result<EchoConfig, ConfigError> = manager.load("nonexistent");
    assert!(matches!(
        result,
        Err(ConfigError::ConfigFileNotFound { .. })
    ));
}

#[test]
fn given_unknown_capsule_when_load_then_schema_not_found_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "someField": "someValue"
    }"#;

    fs::write(manager.config_dir().join("unknown.json"), config_content).unwrap();

    let result: Result<serde_json::Value, ConfigError> = manager.load("unknown");
    assert!(matches!(result, Err(ConfigError::SchemaNotFound { .. })));
}

#[test]
fn given_valid_config_file_when_validate_config_file_then_success() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    let config_path = manager.config_dir().join("test_config.json");
    fs::write(&config_path, config_content).unwrap();

    assert!(manager.validate_config_file("echo", &config_path).is_ok());
}

#[test]
fn given_invalid_config_file_when_validate_config_file_then_validation_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": "not_a_boolean"
    }"#;

    let config_path = manager.config_dir().join("test_config.json");
    fs::write(&config_path, config_content).unwrap();

    let result = manager.validate_config_file("echo", &config_path);
    assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));
}

#[test]
fn given_valid_config_value_when_validate_config_value_then_success() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_value = serde_json::json!({
        "messagePrefix": "Test: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    });

    assert!(manager.validate_config_value("echo", &config_value).is_ok());
}

#[test]
fn given_invalid_config_value_when_validate_config_value_then_validation_error() {
    let (_temp_dir, manager) = setup_test_environment();

    let config_value = serde_json::json!({
        "messagePrefix": "Test: ",
        "enableTrim": "not_a_boolean"
    });

    let result = manager.validate_config_value("echo", &config_value);
    assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));
}

#[test]
fn given_schema_compilation_error_when_load_then_compilation_failed_error() {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");

    fs::create_dir_all(contracts_dir.join("config")).unwrap();
    fs::create_dir_all(&config_dir).unwrap();

    // Create malformed schema
    let bad_schema = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "field": {
                "type": "invalid_type"
            }
        }
    }"#;

    fs::write(contracts_dir.join("config/bad-config.v1.json"), bad_schema).unwrap();

    let config_content = r#"{"field": "value"}"#;
    fs::write(config_dir.join("bad.json"), config_content).unwrap();

    let manager = ConfigManager::with_dirs(contracts_dir, config_dir);
    let result: Result<serde_json::Value, ConfigError> = manager.load("bad");
    assert!(matches!(
        result,
        Err(ConfigError::SchemaCompilationFailed { .. })
    ));
}
