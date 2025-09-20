use crate::secrets::{EnvFileSecretProvider, SecretProvider};
use std::env;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderFactoryError {
    #[error("Unknown provider type: {provider_type}")]
    UnknownProviderType { provider_type: String },

    #[error("Vault configuration error: {message}")]
    VaultConfigError { message: String },

    #[error("Provider initialization failed: {message}")]
    InitializationFailed { message: String },
}

pub struct SecretProviderFactory;

impl SecretProviderFactory {
    /// Create a secret provider based on environment configuration
    ///
    /// Uses CONFIG_SECRETS_PROVIDER environment variable:
    /// - "envfile" (default): EnvFileSecretProvider
    /// - "vault": VaultStubProvider
    pub fn create() -> Result<Box<dyn SecretProvider>, ProviderFactoryError> {
        let provider_type =
            env::var("CONFIG_SECRETS_PROVIDER").unwrap_or_else(|_| "envfile".to_string());

        match provider_type.as_str() {
            "envfile" => Ok(Box::new(EnvFileSecretProvider::new())),
            "vault" => {
                let vault_provider = VaultStubProvider::from_env()
                    .map_err(|e| ProviderFactoryError::VaultConfigError { message: e })?;
                Ok(Box::new(vault_provider))
            }
            other => Err(ProviderFactoryError::UnknownProviderType {
                provider_type: other.to_string(),
            }),
        }
    }

    /// Create a specific provider type for testing or explicit usage
    pub fn create_envfile() -> Box<dyn SecretProvider> {
        Box::new(EnvFileSecretProvider::new())
    }

    pub fn create_vault(
        vault_addr: Option<String>,
        vault_token: Option<String>,
    ) -> Result<Box<dyn SecretProvider>, ProviderFactoryError> {
        let vault_provider = VaultStubProvider::new(vault_addr, vault_token)
            .map_err(|e| ProviderFactoryError::VaultConfigError { message: e })?;
        Ok(Box::new(vault_provider))
    }
}

/// Vault stub provider for testing and development
///
/// This provider simulates a Vault-like interface for secret storage.
/// It can operate in two modes:
/// - File mode: Stores secrets in JSON files under ~/.demon/vault_stub/
/// - HTTP mode: Makes HTTP requests to VAULT_ADDR (minimal implementation)
pub struct VaultStubProvider {
    #[allow(dead_code)] // Stored for future HTTP implementation
    vault_addr: String,
    #[allow(dead_code)] // Stored for future HTTP implementation
    vault_token: Option<String>,
    mode: VaultMode,
}

#[derive(Debug, Clone)]
enum VaultMode {
    File { base_path: std::path::PathBuf },
    Http,
}

impl VaultStubProvider {
    /// Create a new VaultStubProvider with explicit configuration
    pub fn new(vault_addr: Option<String>, vault_token: Option<String>) -> Result<Self, String> {
        let addr = vault_addr.unwrap_or_else(|| "file://vault_stub".to_string());

        let mode = if addr.starts_with("file://") {
            let path = addr.strip_prefix("file://").unwrap();
            let base_path = if path.starts_with('/') {
                std::path::PathBuf::from(path)
            } else {
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".demon")
                    .join(path)
            };
            VaultMode::File { base_path }
        } else if addr.starts_with("http://") || addr.starts_with("https://") {
            VaultMode::Http
        } else {
            return Err(format!(
                "Invalid VAULT_ADDR format: {}. Must start with file://, http://, or https://",
                addr
            ));
        };

        Ok(Self {
            vault_addr: addr,
            vault_token,
            mode,
        })
    }

    /// Create a VaultStubProvider from environment variables
    pub fn from_env() -> Result<Self, String> {
        let vault_addr = env::var("VAULT_ADDR").ok();
        let vault_token = env::var("VAULT_TOKEN").ok();
        Self::new(vault_addr, vault_token)
    }

    /// Validate scope name to prevent directory traversal attacks
    fn validate_scope_name(scope: &str) -> Result<(), String> {
        if scope.is_empty() {
            return Err("Scope name cannot be empty".to_string());
        }

        // Only allow alphanumeric characters, hyphens, and underscores
        if !scope.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(format!(
                "Invalid scope name '{}': only alphanumeric characters, hyphens, and underscores are allowed",
                scope
            ));
        }

        // Additional safety check for specific dangerous patterns
        if scope.contains("..") || scope.starts_with('.') || scope.starts_with('/') || scope.starts_with('\\') {
            return Err(format!(
                "Invalid scope name '{}': path traversal patterns are not allowed",
                scope
            ));
        }

        Ok(())
    }

    /// Resolve a secret using the vault stub
    fn resolve_secret(
        &self,
        scope: &str,
        key: &str,
    ) -> Result<String, crate::secrets::SecretError> {
        Self::validate_scope_name(scope)
            .map_err(|e| crate::secrets::SecretError::SecretsFileError {
                path: scope.to_string(),
                message: e,
            })?;

        match &self.mode {
            VaultMode::File { base_path } => self.resolve_from_file(base_path, scope, key),
            VaultMode::Http => self.resolve_from_http(scope, key),
        }
    }

    /// Store a secret using the vault stub
    fn store_secret(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        Self::validate_scope_name(scope)?;

        match &self.mode {
            VaultMode::File { base_path } => self.store_to_file(base_path, scope, key, value),
            VaultMode::Http => self.store_to_http(scope, key, value),
        }
    }

    /// Delete a secret using the vault stub
    fn delete_secret(&self, scope: &str, key: &str) -> Result<(), String> {
        Self::validate_scope_name(scope)?;

        match &self.mode {
            VaultMode::File { base_path } => self.delete_from_file(base_path, scope, key),
            VaultMode::Http => self.delete_from_http(scope, key),
        }
    }

    // File-based operations
    fn resolve_from_file(
        &self,
        base_path: &std::path::Path,
        scope: &str,
        key: &str,
    ) -> Result<String, crate::secrets::SecretError> {
        use crate::secrets::SecretError;

        let scope_file = base_path.join(format!("{}.json", scope));

        if !scope_file.exists() {
            return Err(SecretError::SecretNotFound {
                scope: scope.to_string(),
                key: key.to_string(),
            });
        }

        let content =
            std::fs::read_to_string(&scope_file).map_err(|e| SecretError::SecretsFileError {
                path: scope_file.to_string_lossy().to_string(),
                message: e.to_string(),
            })?;

        let secrets: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| SecretError::SecretsParseError {
                message: e.to_string(),
            })?;

        if let Some(value) = secrets.get(key).and_then(|v| v.as_str()) {
            Ok(value.to_string())
        } else {
            Err(SecretError::SecretNotFound {
                scope: scope.to_string(),
                key: key.to_string(),
            })
        }
    }

    fn store_to_file(
        &self,
        base_path: &std::path::Path,
        scope: &str,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(base_path)
            .map_err(|e| format!("Failed to create vault stub directory: {}", e))?;

        let scope_file = base_path.join(format!("{}.json", scope));

        // Load existing secrets or create new
        let mut secrets = if scope_file.exists() {
            let content = std::fs::read_to_string(&scope_file)
                .map_err(|e| format!("Failed to read scope file: {}", e))?;

            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse scope file: {}", e))?
        } else {
            serde_json::json!({})
        };

        // Update the secret
        if let Some(obj) = secrets.as_object_mut() {
            obj.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }

        // Write back atomically
        let temp_file = scope_file.with_extension("tmp");
        std::fs::write(&temp_file, serde_json::to_string_pretty(&secrets).unwrap())
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        std::fs::rename(&temp_file, &scope_file)
            .map_err(|e| format!("Failed to rename temp file: {}", e))?;

        // Set appropriate permissions on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&scope_file, permissions)
                .map_err(|e| format!("Failed to set file permissions: {}", e))?;
        }

        Ok(())
    }

    fn delete_from_file(
        &self,
        base_path: &std::path::Path,
        scope: &str,
        key: &str,
    ) -> Result<(), String> {
        let scope_file = base_path.join(format!("{}.json", scope));

        if !scope_file.exists() {
            return Err(format!("Secret {}/{} not found", scope, key));
        }

        let content = std::fs::read_to_string(&scope_file)
            .map_err(|e| format!("Failed to read scope file: {}", e))?;

        let mut secrets: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse scope file: {}", e))?;

        if let Some(obj) = secrets.as_object_mut() {
            if obj.remove(key).is_none() {
                return Err(format!("Secret {}/{} not found", scope, key));
            }

            // If scope is now empty, remove the file
            if obj.is_empty() {
                std::fs::remove_file(&scope_file)
                    .map_err(|e| format!("Failed to remove empty scope file: {}", e))?;
            } else {
                // Write back the updated secrets
                let temp_file = scope_file.with_extension("tmp");
                std::fs::write(&temp_file, serde_json::to_string_pretty(&secrets).unwrap())
                    .map_err(|e| format!("Failed to write temp file: {}", e))?;

                std::fs::rename(&temp_file, &scope_file)
                    .map_err(|e| format!("Failed to rename temp file: {}", e))?;
            }
        }

        Ok(())
    }

    // HTTP-based operations (minimal implementation)
    fn resolve_from_http(
        &self,
        scope: &str,
        key: &str,
    ) -> Result<String, crate::secrets::SecretError> {
        // This is a minimal stub - in a real implementation you'd use an HTTP client
        // For now, just return an error indicating HTTP mode is not fully implemented
        tracing::warn!(
            "HTTP mode vault stub accessed for {}/{} - not fully implemented",
            scope,
            key
        );
        Err(crate::secrets::SecretError::SecretNotFound {
            scope: scope.to_string(),
            key: key.to_string(),
        })
    }

    fn store_to_http(&self, scope: &str, key: &str, _value: &str) -> Result<(), String> {
        tracing::warn!(
            "HTTP mode vault stub store operation for {}/{} - not fully implemented",
            scope,
            key
        );
        Err("HTTP mode not fully implemented".to_string())
    }

    fn delete_from_http(&self, scope: &str, key: &str) -> Result<(), String> {
        tracing::warn!(
            "HTTP mode vault stub delete operation for {}/{} - not fully implemented",
            scope,
            key
        );
        Err("HTTP mode not fully implemented".to_string())
    }
}

impl SecretProvider for VaultStubProvider {
    fn resolve(&self, scope: &str, key: &str) -> Result<String, crate::secrets::SecretError> {
        tracing::debug!("VaultStubProvider resolving secret {}/{}", scope, key);
        self.resolve_secret(scope, key)
    }
}

// Public interface for CLI operations
impl VaultStubProvider {
    pub fn put(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        tracing::debug!("VaultStubProvider storing secret {}/{}", scope, key);
        self.store_secret(scope, key, value)
    }

    pub fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        tracing::debug!("VaultStubProvider deleting secret {}/{}", scope, key);
        self.delete_secret(scope, key)
    }

    pub fn list(
        &self,
        scope: Option<&str>,
    ) -> Result<std::collections::HashMap<String, std::collections::HashMap<String, String>>, String>
    {
        // Validate scope name if provided
        if let Some(scope_name) = scope {
            Self::validate_scope_name(scope_name)?;
        }

        match &self.mode {
            VaultMode::File { base_path } => self.list_from_file(base_path, scope),
            VaultMode::Http => {
                tracing::warn!("HTTP mode vault stub list operation - not fully implemented");
                Err("HTTP mode not fully implemented".to_string())
            }
        }
    }

    fn list_from_file(
        &self,
        base_path: &std::path::Path,
        scope_filter: Option<&str>,
    ) -> Result<std::collections::HashMap<String, std::collections::HashMap<String, String>>, String>
    {
        use std::collections::HashMap;

        let mut result = HashMap::new();

        if !base_path.exists() {
            return Ok(result);
        }

        let entries = std::fs::read_dir(base_path)
            .map_err(|e| format!("Failed to read vault stub directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let scope_name = file_stem.to_string();

                    // Apply scope filter if provided
                    if let Some(filter) = scope_filter {
                        if scope_name != filter {
                            continue;
                        }
                    }

                    let content = std::fs::read_to_string(&path).map_err(|e| {
                        format!("Failed to read scope file {}: {}", path.display(), e)
                    })?;

                    let secrets: serde_json::Value =
                        serde_json::from_str(&content).map_err(|e| {
                            format!("Failed to parse scope file {}: {}", path.display(), e)
                        })?;

                    if let Some(obj) = secrets.as_object() {
                        let mut scope_secrets = HashMap::new();
                        for (key, value) in obj {
                            if let Some(str_value) = value.as_str() {
                                // Redact the value for listing
                                scope_secrets.insert(
                                    key.clone(),
                                    crate::secrets_store::redact_value(str_value),
                                );
                            }
                        }
                        if !scope_secrets.is_empty() {
                            result.insert(scope_name, scope_secrets);
                        }
                    }
                }
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_factory_default_envfile() {
        // Default should be envfile when no env var is set
        std::env::remove_var("CONFIG_SECRETS_PROVIDER");
        let provider = SecretProviderFactory::create().unwrap();
        // We can't easily test the concrete type, but we can test behavior
        // This just ensures it doesn't panic
        drop(provider);
    }

    #[test]
    fn test_factory_vault_creation() {
        std::env::set_var("CONFIG_SECRETS_PROVIDER", "vault");
        std::env::set_var("VAULT_ADDR", "file://test_vault");

        let provider = SecretProviderFactory::create().unwrap();
        drop(provider);

        std::env::remove_var("CONFIG_SECRETS_PROVIDER");
        std::env::remove_var("VAULT_ADDR");
    }

    #[test]
    fn test_vault_stub_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let vault_addr = format!("file://{}", temp_dir.path().display());

        let provider = VaultStubProvider::new(Some(vault_addr), None).unwrap();

        // Test store and resolve
        provider.put("test", "key1", "value1").unwrap();
        let result = provider.resolve("test", "key1").unwrap();
        assert_eq!(result, "value1");

        // Test delete
        provider.delete("test", "key1").unwrap();
        assert!(provider.resolve("test", "key1").is_err());
    }

    #[test]
    fn test_vault_stub_list() {
        let temp_dir = TempDir::new().unwrap();
        let vault_addr = format!("file://{}", temp_dir.path().display());

        let provider = VaultStubProvider::new(Some(vault_addr), None).unwrap();

        // Store some test secrets
        provider
            .put("scope1", "key1", "verylongsecretvalue")
            .unwrap();
        provider.put("scope1", "key2", "short").unwrap();
        provider.put("scope2", "key1", "anothersecret").unwrap();

        // Test list all
        let all_secrets = provider.list(None).unwrap();
        assert_eq!(all_secrets.len(), 2);
        assert!(all_secrets.contains_key("scope1"));
        assert!(all_secrets.contains_key("scope2"));

        // Values should be redacted
        assert_eq!(all_secrets["scope1"]["key1"], "ver***");
        assert_eq!(all_secrets["scope1"]["key2"], "***");

        // Test list specific scope
        let scope1_secrets = provider.list(Some("scope1")).unwrap();
        assert_eq!(scope1_secrets.len(), 1);
        assert!(scope1_secrets.contains_key("scope1"));
        assert_eq!(scope1_secrets["scope1"].len(), 2);
    }

    #[test]
    fn test_vault_stub_invalid_addr() {
        let result = VaultStubProvider::new(Some("invalid://addr".to_string()), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_vault_stub_scope_validation_prevents_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let vault_addr = format!("file://{}", temp_dir.path().display());
        let provider = VaultStubProvider::new(Some(vault_addr), None).unwrap();

        // Test dangerous scope names that could cause directory traversal
        let dangerous_scopes = vec![
            "../etc",
            "../../tmp",
            ".ssh",
            "/etc/passwd",
            "scope/with/slash",
            "scope..parent",
            "scope with spaces",
            "scope!@#$",
            "",  // empty scope
        ];

        for dangerous_scope in dangerous_scopes {
            // All these operations should fail with scope validation errors
            assert!(provider.put(dangerous_scope, "key", "value").is_err(),
                    "put should fail for dangerous scope: {}", dangerous_scope);
            assert!(provider.resolve(dangerous_scope, "key").is_err(),
                    "resolve should fail for dangerous scope: {}", dangerous_scope);
            assert!(provider.delete(dangerous_scope, "key").is_err(),
                    "delete should fail for dangerous scope: {}", dangerous_scope);
            assert!(provider.list(Some(dangerous_scope)).is_err(),
                    "list should fail for dangerous scope: {}", dangerous_scope);
        }

        // Test valid scope names should work
        let valid_scopes = vec!["test", "api", "database", "my-app", "app_1", "Test123"];

        for valid_scope in valid_scopes {
            // These should all succeed
            assert!(provider.put(valid_scope, "key", "value").is_ok(),
                    "put should succeed for valid scope: {}", valid_scope);
            assert!(provider.resolve(valid_scope, "key").is_ok(),
                    "resolve should succeed for valid scope: {}", valid_scope);
            assert!(provider.delete(valid_scope, "key").is_ok(),
                    "delete should succeed for valid scope: {}", valid_scope);
        }
    }
}
