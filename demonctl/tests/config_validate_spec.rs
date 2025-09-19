use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn setup_test_environment() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");

    fs::create_dir_all(contracts_dir.join("config")).unwrap();

    // Create echo config schema in temp directory
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

    temp_dir
}

#[test]
fn given_valid_config_file_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    let valid_config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    fs::write(&config_file, valid_config).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}

#[test]
fn given_invalid_config_file_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    let invalid_config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": "not_a_boolean"
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    fs::write(&config_file, invalid_config).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "✗ Invalid config for capsule 'echo'",
    ));
}

#[test]
fn given_missing_required_field_when_validate_config_then_failure_with_details() {
    let temp_dir = setup_test_environment();

    let invalid_config = r#"{
        "messagePrefix": "Echo: "
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    fs::write(&config_file, invalid_config).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "✗ Invalid config for capsule 'echo'",
        ))
        .stderr(predicate::str::contains("enableTrim"));
}

#[test]
fn given_valid_config_with_explicit_schema_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    let valid_config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    let config_file = temp_dir.path().join("custom_name.json");
    fs::write(&config_file, valid_config).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
        "--schema",
        "echo",
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}

#[test]
fn given_nonexistent_schema_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    let config = r#"{
        "someField": "someValue"
    }"#;

    let config_file = temp_dir.path().join("unknown_config.json");
    fs::write(&config_file, config).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
    ]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Schema not found for capsule: unknown",
    ));
}

#[test]
fn given_valid_config_via_stdin_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    let valid_config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .args([
            "contracts",
            "validate-config",
            "--stdin",
            "--schema",
            "echo",
        ])
        .write_stdin(valid_config);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}

#[test]
fn given_invalid_config_via_stdin_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    let invalid_config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": "not_a_boolean"
    }"#;

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .args([
            "contracts",
            "validate-config",
            "--stdin",
            "--schema",
            "echo",
        ])
        .write_stdin(invalid_config);

    cmd.assert().failure().stderr(predicate::str::contains(
        "✗ Invalid config for capsule 'echo'",
    ));
}

#[test]
fn given_stdin_without_schema_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    let config = r#"{
        "messagePrefix": "Echo: ",
        "enableTrim": true
    }"#;

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["contracts", "validate-config", "--stdin"])
        .write_stdin(config);

    cmd.assert().failure().stderr(predicate::str::contains(
        "--schema is required when reading from stdin",
    ));
}

#[test]
fn given_no_file_or_stdin_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .args(["contracts", "validate-config"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "Must specify either a file path or --stdin",
    ));
}

#[test]
fn given_config_with_secrets_and_secrets_file_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    // Create a config with secret references
    let config_with_secrets = r#"{
        "messagePrefix": "secret://app/prefix",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    // Create a secrets file
    let secrets_content = r#"{
        "app": {
            "prefix": "Secret Prefix: "
        }
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    let secrets_file = temp_dir.path().join("secrets.json");
    fs::write(&config_file, config_with_secrets).unwrap();
    fs::write(&secrets_file, secrets_content).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
        "--secrets-file",
        &secrets_file.to_string_lossy(),
    ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}

#[test]
fn given_config_with_missing_secret_when_validate_config_then_failure() {
    let temp_dir = setup_test_environment();

    // Create a config with secret references that don't exist
    let config_with_secrets = r#"{
        "messagePrefix": "secret://app/missing_secret",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    // Create an empty secrets file
    let secrets_content = r#"{
        "app": {}
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    let secrets_file = temp_dir.path().join("secrets.json");
    fs::write(&config_file, config_with_secrets).unwrap();
    fs::write(&secrets_file, secrets_content).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path()).args([
        "contracts",
        "validate-config",
        &config_file.to_string_lossy(),
        "--secrets-file",
        &secrets_file.to_string_lossy(),
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("✗ Secret resolution failed"))
        .stderr(predicate::str::contains(
            "Secret not found: app/missing_secret",
        ));
}

#[test]
fn given_config_with_secrets_via_env_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    // Create a config with secret references
    let config_with_secrets = r#"{
        "messagePrefix": "secret://test/prefix",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    let config_file = temp_dir.path().join("echo_config.json");
    fs::write(&config_file, config_with_secrets).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("SECRET_TEST_PREFIX", "Env Secret: ")
        .args([
            "contracts",
            "validate-config",
            &config_file.to_string_lossy(),
        ]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}

#[test]
fn given_config_with_secrets_via_stdin_when_validate_config_then_success() {
    let temp_dir = setup_test_environment();

    // Create a config with secret references
    let config_with_secrets = r#"{
        "messagePrefix": "secret://app/prefix",
        "enableTrim": true,
        "maxMessageLength": 500,
        "outputFormat": "plain"
    }"#;

    // Create a secrets file
    let secrets_content = r#"{
        "app": {
            "prefix": "Stdin Secret: "
        }
    }"#;

    let secrets_file = temp_dir.path().join("secrets.json");
    fs::write(&secrets_file, secrets_content).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.current_dir(temp_dir.path())
        .args([
            "contracts",
            "validate-config",
            "--stdin",
            "--schema",
            "echo",
            "--secrets-file",
            &secrets_file.to_string_lossy(),
        ])
        .write_stdin(config_with_secrets);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Valid config for capsule: echo"));
}
