use config_loader::{SecretProvider, SecretProviderFactory, VaultStubProvider};
use runtime::link::router::Router;
use std::env;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_factory_creates_envfile_provider_by_default() {
    // Ensure no CONFIG_SECRETS_PROVIDER env var is set
    env::remove_var("CONFIG_SECRETS_PROVIDER");

    let provider = SecretProviderFactory::create().unwrap();
    // Can't easily test the concrete type, but we can test that it doesn't panic
    drop(provider);
}

#[test]
fn test_factory_creates_vault_provider_when_configured() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().join("vault_test");

    env::set_var("CONFIG_SECRETS_PROVIDER", "vault");
    env::set_var("VAULT_ADDR", format!("file://{}", vault_path.display()));

    let provider = SecretProviderFactory::create().unwrap();
    drop(provider);

    // Clean up
    env::remove_var("CONFIG_SECRETS_PROVIDER");
    env::remove_var("VAULT_ADDR");
}

#[test]
fn test_router_uses_factory_for_provider_selection() {
    let temp_dir = TempDir::new().unwrap();
    let vault_path = temp_dir.path().join("vault_test");

    // Test with vault provider
    env::set_var("CONFIG_SECRETS_PROVIDER", "vault");
    env::set_var("VAULT_ADDR", format!("file://{}", vault_path.display()));

    let router = Router::new();
    // This should not panic and should successfully create router with vault provider
    drop(router);

    // Test with envfile provider
    env::set_var("CONFIG_SECRETS_PROVIDER", "envfile");
    env::remove_var("VAULT_ADDR");

    let router = Router::new();
    drop(router);

    // Clean up
    env::remove_var("CONFIG_SECRETS_PROVIDER");
}

#[tokio::test]
async fn test_router_handles_vault_secret_resolution_failure() {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");
    let vault_dir = temp_dir.path().join("vault");

    // Create test directories
    fs::create_dir_all(contracts_dir.join("config")).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&vault_dir).unwrap();

    // Create echo schema
    let schema_content = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "messagePrefix": { "type": "string", "default": "" },
            "enableTrim": { "type": "boolean", "default": true },
            "apiKey": { "type": "string" }
        },
        "required": ["messagePrefix", "enableTrim", "apiKey"],
        "additionalProperties": false
    }"#;

    fs::write(
        contracts_dir.join("config/echo-config.v1.json"),
        schema_content,
    )
    .unwrap();

    // Create config with secret reference
    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": true,
        "apiKey": "secret://api/key"
    }"#;

    fs::write(config_dir.join("echo.json"), config_content).unwrap();

    // Set up vault provider without the required secret
    env::set_var("CONFIG_SECRETS_PROVIDER", "vault");
    env::set_var("VAULT_ADDR", format!("file://{}", vault_dir.display()));

    let config_manager = config_loader::ConfigManager::with_dirs(contracts_dir, config_dir);
    let router = Router::with_config_manager(config_manager);

    // This should fail due to missing secret
    let result = router
        .dispatch(
            "echo",
            &serde_json::json!({"message": "test"}),
            "test-run",
            "test-ritual",
        )
        .await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Configuration validation failed"));

    // Clean up
    env::remove_var("CONFIG_SECRETS_PROVIDER");
    env::remove_var("VAULT_ADDR");
}

#[tokio::test]
async fn test_router_succeeds_with_vault_secret_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let contracts_dir = temp_dir.path().join("contracts");
    let config_dir = temp_dir.path().join("config");
    let vault_dir = temp_dir.path().join("vault");

    // Create test directories
    fs::create_dir_all(contracts_dir.join("config")).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&vault_dir).unwrap();

    // Create echo schema
    let schema_content = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "messagePrefix": { "type": "string", "default": "" },
            "enableTrim": { "type": "boolean", "default": true },
            "apiKey": { "type": "string" }
        },
        "required": ["messagePrefix", "enableTrim", "apiKey"],
        "additionalProperties": false
    }"#;

    fs::write(
        contracts_dir.join("config/echo-config.v1.json"),
        schema_content,
    )
    .unwrap();

    // Create config with secret reference
    let config_content = r#"{
        "messagePrefix": "Test: ",
        "enableTrim": true,
        "apiKey": "secret://api/key"
    }"#;

    fs::write(config_dir.join("echo.json"), config_content).unwrap();

    // Set up vault provider and store the required secret
    env::set_var("CONFIG_SECRETS_PROVIDER", "vault");
    env::set_var("VAULT_ADDR", format!("file://{}", vault_dir.display()));

    let vault_provider = VaultStubProvider::from_env().unwrap();
    vault_provider.put("api", "key", "test-api-key").unwrap();

    let config_manager = config_loader::ConfigManager::with_dirs(contracts_dir, config_dir);
    let router = Router::with_config_manager(config_manager);

    // This should succeed with the secret resolved
    let result = router
        .dispatch(
            "echo",
            &serde_json::json!({"message": "test"}),
            "test-run",
            "test-ritual",
        )
        .await;

    assert!(result.is_ok());

    // Clean up
    env::remove_var("CONFIG_SECRETS_PROVIDER");
    env::remove_var("VAULT_ADDR");
}

#[test]
fn test_vault_stub_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let vault_addr = format!("file://{}", temp_dir.path().display());

    let provider = VaultStubProvider::new(Some(vault_addr), None).unwrap();

    // Test put and resolve
    provider.put("test", "key1", "value1").unwrap();
    provider.put("test", "key2", "value2").unwrap();
    provider.put("other", "key1", "other-value").unwrap();

    assert_eq!(provider.resolve("test", "key1").unwrap(), "value1");
    assert_eq!(provider.resolve("test", "key2").unwrap(), "value2");
    assert_eq!(provider.resolve("other", "key1").unwrap(), "other-value");

    // Test list functionality
    let all_secrets = provider.list(None).unwrap();
    assert_eq!(all_secrets.len(), 2);
    assert!(all_secrets.contains_key("test"));
    assert!(all_secrets.contains_key("other"));

    // Test scope filtering
    let test_secrets = provider.list(Some("test")).unwrap();
    assert_eq!(test_secrets.len(), 1);
    assert!(test_secrets.contains_key("test"));
    assert_eq!(test_secrets["test"].len(), 2);

    // Test delete
    provider.delete("test", "key1").unwrap();
    assert!(provider.resolve("test", "key1").is_err());
    assert_eq!(provider.resolve("test", "key2").unwrap(), "value2");

    // Delete all keys in scope - scope should be removed
    provider.delete("test", "key2").unwrap();
    let remaining_secrets = provider.list(None).unwrap();
    assert_eq!(remaining_secrets.len(), 1);
    assert!(remaining_secrets.contains_key("other"));
}

#[test]
fn test_vault_stub_invalid_configurations() {
    // Test invalid vault addr
    let result = VaultStubProvider::new(Some("invalid://protocol".to_string()), None);
    assert!(result.is_err());

    // Test missing env vars
    env::remove_var("VAULT_ADDR");
    env::remove_var("VAULT_TOKEN");

    let provider = VaultStubProvider::from_env().unwrap();
    // Should default to file://vault_stub under home dir
    drop(provider);
}

#[test]
fn test_vault_stub_permission_handling() {
    let temp_dir = TempDir::new().unwrap();
    let vault_addr = format!("file://{}", temp_dir.path().display());

    let provider = VaultStubProvider::new(Some(vault_addr), None).unwrap();

    provider
        .put("secure", "password", "very-secret-password")
        .unwrap();

    // On Unix systems, check that the file has proper permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let scope_file = temp_dir.path().join("secure.json");
        if scope_file.exists() {
            let metadata = fs::metadata(&scope_file).unwrap();
            let permissions = metadata.permissions().mode() & 0o777;
            assert_eq!(
                permissions, 0o600,
                "Vault stub files should have 600 permissions"
            );
        }
    }
}
