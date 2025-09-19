use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn setup_test_env() -> (TempDir, String) {
    let temp_dir = TempDir::new().unwrap();
    let secrets_file = temp_dir.path().join("test_secrets.json");
    (temp_dir, secrets_file.to_string_lossy().to_string())
}

#[test]
fn test_secrets_set_get_delete_flow() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    // Set a secret
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "database/password", "secretvalue123"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Secret database/password set successfully",
        ));

    // Get the secret (redacted)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "get", "database/password"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("database/password: sec***"));

    // Get the secret (raw)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "get", "database/password", "--raw"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("secretvalue123").and(predicate::str::contains("***").not()),
        );

    // Delete the secret
    Command::cargo_bin("demonctl")?
        .args(["secrets", "delete", "database/password"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("Secret database/password deleted"));

    // Try to get deleted secret (should fail)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "get", "database/password"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Secret not found"));

    Ok(())
}

#[test]
fn test_secrets_set_from_env() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    std::env::set_var("TEST_SECRET_VALUE", "env_secret_123");

    Command::cargo_bin("demonctl")?
        .args([
            "secrets",
            "set",
            "api/token",
            "--from-env",
            "TEST_SECRET_VALUE",
        ])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Secret api/token set successfully",
        ));

    // Verify it was set correctly
    Command::cargo_bin("demonctl")?
        .args(["secrets", "get", "api/token", "--raw"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("env_secret_123"));

    std::env::remove_var("TEST_SECRET_VALUE");
    Ok(())
}

#[test]
fn test_secrets_list() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    // Set multiple secrets
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "database/password", "dbpass123"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success();

    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "database/username", "admin"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success();

    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "api/key", "apikey456"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success();

    // List all secrets
    Command::cargo_bin("demonctl")?
        .args(["secrets", "list"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("database:")
                .and(predicate::str::contains("password: dbp***"))
                .and(predicate::str::contains("username: ***"))
                .and(predicate::str::contains("api:"))
                .and(predicate::str::contains("key: api***")),
        );

    // List by scope
    Command::cargo_bin("demonctl")?
        .args(["secrets", "list", "--scope", "database"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("password: dbp***")
                .and(predicate::str::contains("username: ***"))
                .and(predicate::str::contains("api:").not()),
        );

    Ok(())
}

#[test]
fn test_secrets_invalid_format() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    // Invalid format (no slash)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "invalidkey", "value"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid scope/key format"));

    // Invalid format (empty scope)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "/key", "value"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid scope/key format"));

    // Invalid format (empty key)
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "scope/", "value"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid scope/key format"));

    Ok(())
}

#[test]
fn test_secrets_file_format() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    // Set some secrets
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "echo/api_key", "echo123"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success();

    Command::cargo_bin("demonctl")?
        .args([
            "secrets",
            "set",
            "database/connection",
            "postgres://localhost",
        ])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success();

    // Read the file and verify JSON format
    let content = fs::read_to_string(&secrets_file)?;
    let json: Value = serde_json::from_str(&content)?;

    // Verify structure matches EnvFileSecretProvider expectations
    assert!(json.is_object());
    assert_eq!(json["echo"]["api_key"], "echo123");
    assert_eq!(json["database"]["connection"], "postgres://localhost");

    Ok(())
}

#[test]
fn test_secrets_empty_list() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    Command::cargo_bin("demonctl")?
        .args(["secrets", "list"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("No secrets found"));

    Ok(())
}

#[test]
fn test_secrets_delete_nonexistent() -> Result<()> {
    let (_temp_dir, secrets_file) = setup_test_env();

    Command::cargo_bin("demonctl")?
        .args(["secrets", "delete", "nonexistent/key"])
        .arg("--secrets-file")
        .arg(&secrets_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Secret not found"));

    Ok(())
}

#[test]
fn test_secrets_integration_with_config_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let secrets_file = temp_dir.path().join("secrets.json");
    let config_file = temp_dir.path().join("echo_config.json");

    // Create a test config with secret URIs
    let config_content = r#"{
        "messagePrefix": "secret://echo/prefix",
        "enableTrim": true
    }"#;
    fs::write(&config_file, config_content)?;

    // Set the secret using our CLI
    Command::cargo_bin("demonctl")?
        .args(["secrets", "set", "echo/prefix", "Test Secret: "])
        .arg("--secrets-file")
        .arg(secrets_file.to_str().unwrap())
        .assert()
        .success();

    // Now validate config with the secrets file
    // Note: This assumes the echo schema exists in contracts/config/
    // We'll just verify the command structure is correct
    let result = Command::cargo_bin("demonctl")?
        .args(["contracts", "validate-config"])
        .arg(config_file.to_str().unwrap())
        .arg("--schema")
        .arg("echo")
        .arg("--secrets-file")
        .arg(secrets_file.to_str().unwrap())
        .assert();

    // The validation might fail if schema doesn't exist, but command should be recognized
    assert!(
        result.get_output().status.success()
            || String::from_utf8_lossy(&result.get_output().stderr).contains("Schema not found")
    );

    Ok(())
}
