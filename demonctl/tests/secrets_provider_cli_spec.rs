use std::env;
use std::process::Command;
use tempfile::TempDir;

fn demonctl_binary() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_demonctl"));
    cmd.env_clear(); // Start with clean environment
    cmd
}

#[test]
fn test_secrets_set_get_with_envfile_provider() {
    let temp_dir = TempDir::new().unwrap();
    let secrets_file = temp_dir.path().join("test_secrets.json");

    // Set a secret using envfile provider
    let output = demonctl_binary()
        .args([
            "secrets",
            "set",
            "test/key",
            "--secrets-file",
            secrets_file.to_str().unwrap(),
            "--provider",
            "envfile",
            "test-value",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Set command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("✓ Secret test/key set successfully"));

    // Get the secret back
    let output = demonctl_binary()
        .args([
            "secrets",
            "get",
            "test/key",
            "--secrets-file",
            secrets_file.to_str().unwrap(),
            "--provider",
            "envfile",
            "--raw",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Get command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test-value");
}

#[test]
fn test_secrets_set_get_with_vault_provider() {
    let temp_dir = TempDir::new().unwrap();
    let vault_dir = temp_dir.path().join("vault");

    // Set a secret using vault provider
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args([
            "secrets",
            "set",
            "api/token",
            "--provider",
            "vault",
            "vault-test-token",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Set command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("✓ Secret api/token set successfully in vault"));

    // Get the secret back
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args([
            "secrets",
            "get",
            "api/token",
            "--provider",
            "vault",
            "--raw",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Get command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "vault-test-token"
    );
}

#[test]
fn test_secrets_list_with_vault_provider() {
    let temp_dir = TempDir::new().unwrap();
    let vault_dir = temp_dir.path().join("vault");

    // Set multiple secrets
    for (scope, key, value) in [
        ("db", "password", "db-secret"),
        ("db", "username", "admin"),
        ("api", "key", "api-secret"),
    ] {
        let output = demonctl_binary()
            .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
            .args([
                "secrets",
                "set",
                &format!("{}/{}", scope, key),
                "--provider",
                "vault",
                value,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "Set command failed for {}/{}: {}",
            scope,
            key,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // List all secrets
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args(["secrets", "list", "--provider", "vault"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "List command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Secrets (vault):"));
    assert!(stdout.contains("db:"));
    assert!(stdout.contains("api:"));
    assert!(stdout.contains("password: db-***")); // Should be redacted
    assert!(stdout.contains("key: api***")); // Should be redacted

    // List specific scope
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args(["secrets", "list", "--scope", "db", "--provider", "vault"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "List scope command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Secrets in scope 'db' (vault):"));
    assert!(stdout.contains("password: db-***"));
    assert!(stdout.contains("username: ***"));
    assert!(!stdout.contains("api:"));
}

#[test]
fn test_secrets_delete_with_vault_provider() {
    let temp_dir = TempDir::new().unwrap();
    let vault_dir = temp_dir.path().join("vault");

    // Set a secret
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args([
            "secrets",
            "set",
            "temp/data",
            "--provider",
            "vault",
            "temporary-value",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    // Verify it exists
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args([
            "secrets",
            "get",
            "temp/data",
            "--provider",
            "vault",
            "--raw",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "temporary-value"
    );

    // Delete the secret
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args(["secrets", "delete", "temp/data", "--provider", "vault"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("✓ Secret temp/data deleted from vault")
    );

    // Verify it's gone
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args(["secrets", "get", "temp/data", "--provider", "vault"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to get secret from vault"));
}

#[test]
fn test_secrets_file_warning_with_vault_provider() {
    let temp_dir = TempDir::new().unwrap();
    let vault_dir = temp_dir.path().join("vault");
    let dummy_secrets_file = temp_dir.path().join("dummy.json");

    // Try to use --secrets-file with vault provider (should warn)
    let output = demonctl_binary()
        .env("VAULT_ADDR", format!("file://{}", vault_dir.display()))
        .args([
            "secrets",
            "set",
            "test/key",
            "--secrets-file",
            dummy_secrets_file.to_str().unwrap(),
            "--provider",
            "vault",
            "test-value",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("⚠ Warning: --secrets-file is ignored when using vault provider"));
}

#[test]
fn test_vault_provider_initialization_failure() {
    // Try to use vault provider without proper configuration
    let output = demonctl_binary()
        .env("VAULT_ADDR", "invalid://protocol")
        .args(["secrets", "get", "test/key", "--provider", "vault"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to initialize vault provider"));
}

#[test]
fn test_provider_help_text() {
    let output = demonctl_binary()
        .args(["secrets", "set", "--help"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--provider"));
    assert!(stdout.contains("Secret provider to use"));
    assert!(stdout.contains("envfile"));
    assert!(stdout.contains("vault"));
}

#[test]
fn test_envfile_provider_backwards_compatibility() {
    let temp_dir = TempDir::new().unwrap();
    let secrets_file = temp_dir.path().join("compat_test.json");

    // Set secret without specifying provider (should default to envfile)
    let output = demonctl_binary()
        .args([
            "secrets",
            "set",
            "compat/test",
            "--secrets-file",
            secrets_file.to_str().unwrap(),
            "compat-value",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    // Get secret with explicit envfile provider
    let output = demonctl_binary()
        .args([
            "secrets",
            "get",
            "compat/test",
            "--secrets-file",
            secrets_file.to_str().unwrap(),
            "--provider",
            "envfile",
            "--raw",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "compat-value"
    );

    // Get secret without specifying provider (should still work)
    let output = demonctl_binary()
        .args([
            "secrets",
            "get",
            "compat/test",
            "--secrets-file",
            secrets_file.to_str().unwrap(),
            "--raw",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "compat-value"
    );
}
