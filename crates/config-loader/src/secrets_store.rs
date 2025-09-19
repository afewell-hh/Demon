use anyhow::Result;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Failed to read secrets file: {message}")]
    FileReadError { message: String },

    #[error("Failed to write secrets file: {message}")]
    FileWriteError { message: String },

    #[error("Invalid JSON format: {message}")]
    JsonError { message: String },

    #[error("Secret not found: {scope}/{key}")]
    SecretNotFound { scope: String, key: String },

    #[error("Invalid scope/key format: {input}")]
    InvalidFormat { input: String },
}

/// Manages secrets storage in JSON files with atomic writes
pub struct SecretsStore {
    secrets_file: PathBuf,
}

impl SecretsStore {
    /// Create a new SecretsStore with the specified file path
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            secrets_file: path.into(),
        }
    }

    /// Create a SecretsStore using the default location from env or .demon/secrets.json
    pub fn default_location() -> Self {
        let path = std::env::var("CONFIG_SECRETS_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".demon/secrets.json"));
        Self::new(path)
    }

    /// Get the path to the secrets file
    pub fn path(&self) -> &Path {
        &self.secrets_file
    }

    /// Load secrets from the file
    pub fn load(&self) -> Result<HashMap<String, HashMap<String, String>>, StoreError> {
        if !self.secrets_file.exists() {
            debug!("Secrets file does not exist, returning empty store");
            return Ok(HashMap::new());
        }

        let content =
            fs::read_to_string(&self.secrets_file).map_err(|e| StoreError::FileReadError {
                message: format!("{}: {}", self.secrets_file.display(), e),
            })?;

        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }

        let json_value: Value =
            serde_json::from_str(&content).map_err(|e| StoreError::JsonError {
                message: e.to_string(),
            })?;

        let mut secrets = HashMap::new();
        if let Some(obj) = json_value.as_object() {
            for (scope, scope_value) in obj {
                if let Some(scope_obj) = scope_value.as_object() {
                    let mut scope_secrets = HashMap::new();
                    for (key, value) in scope_obj {
                        if let Some(string_value) = value.as_str() {
                            scope_secrets.insert(key.clone(), string_value.to_string());
                        }
                    }
                    if !scope_secrets.is_empty() {
                        secrets.insert(scope.clone(), scope_secrets);
                    }
                }
            }
        }

        Ok(secrets)
    }

    /// Save secrets to file atomically
    pub fn save(
        &self,
        secrets: &HashMap<String, HashMap<String, String>>,
    ) -> Result<(), StoreError> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.secrets_file.parent() {
            fs::create_dir_all(parent).map_err(|e| StoreError::FileWriteError {
                message: format!("Failed to create directory {}: {}", parent.display(), e),
            })?;
        }

        // Convert to JSON Value
        let mut json_obj = Map::new();
        for (scope, scope_secrets) in secrets {
            let mut scope_obj = Map::new();
            for (key, value) in scope_secrets {
                scope_obj.insert(key.clone(), Value::String(value.clone()));
            }
            if !scope_obj.is_empty() {
                json_obj.insert(scope.clone(), Value::Object(scope_obj));
            }
        }

        let json_value = Value::Object(json_obj);
        let json_string =
            serde_json::to_string_pretty(&json_value).map_err(|e| StoreError::JsonError {
                message: e.to_string(),
            })?;

        // Write atomically using a temp file
        let parent_dir = self.secrets_file.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_file =
            NamedTempFile::new_in(parent_dir).map_err(|e| StoreError::FileWriteError {
                message: format!("Failed to create temp file: {}", e),
            })?;

        temp_file
            .write_all(json_string.as_bytes())
            .map_err(|e| StoreError::FileWriteError {
                message: format!("Failed to write to temp file: {}", e),
            })?;

        temp_file.flush().map_err(|e| StoreError::FileWriteError {
            message: format!("Failed to flush temp file: {}", e),
        })?;

        // Set appropriate permissions (0600) on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            temp_file
                .as_file()
                .set_permissions(permissions)
                .map_err(|e| StoreError::FileWriteError {
                    message: format!("Failed to set file permissions: {}", e),
                })?;
        }

        // Atomically replace the original file
        temp_file
            .persist(&self.secrets_file)
            .map_err(|e| StoreError::FileWriteError {
                message: format!("Failed to persist temp file: {}", e),
            })?;

        debug!("Secrets saved to {}", self.secrets_file.display());
        Ok(())
    }

    /// Set a secret value
    pub fn set(&self, scope: &str, key: &str, value: &str) -> Result<(), StoreError> {
        let mut secrets = self.load()?;
        secrets
            .entry(scope.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), value.to_string());
        self.save(&secrets)
    }

    /// Get a secret value
    pub fn get(&self, scope: &str, key: &str) -> Result<String, StoreError> {
        let secrets = self.load()?;
        secrets
            .get(scope)
            .and_then(|scope_secrets| scope_secrets.get(key))
            .cloned()
            .ok_or_else(|| StoreError::SecretNotFound {
                scope: scope.to_string(),
                key: key.to_string(),
            })
    }

    /// Delete a secret
    pub fn delete(&self, scope: &str, key: &str) -> Result<(), StoreError> {
        let mut secrets = self.load()?;

        let removed = secrets
            .get_mut(scope)
            .and_then(|scope_secrets| scope_secrets.remove(key))
            .is_some();

        if !removed {
            return Err(StoreError::SecretNotFound {
                scope: scope.to_string(),
                key: key.to_string(),
            });
        }

        // Remove empty scopes
        if let Some(scope_secrets) = secrets.get(scope) {
            if scope_secrets.is_empty() {
                secrets.remove(scope);
            }
        }

        self.save(&secrets)
    }

    /// List all secrets (returns scope -> key -> redacted value)
    pub fn list(&self) -> Result<HashMap<String, HashMap<String, String>>, StoreError> {
        let secrets = self.load()?;
        let mut redacted = HashMap::new();

        for (scope, scope_secrets) in secrets {
            let mut redacted_scope = HashMap::new();
            for (key, value) in scope_secrets {
                redacted_scope.insert(key, redact_value(&value));
            }
            redacted.insert(scope, redacted_scope);
        }

        Ok(redacted)
    }

    /// List secrets for a specific scope
    pub fn list_scope(&self, scope: &str) -> Result<HashMap<String, String>, StoreError> {
        let secrets = self.load()?;

        if let Some(scope_secrets) = secrets.get(scope) {
            let mut redacted = HashMap::new();
            for (key, value) in scope_secrets {
                redacted.insert(key.clone(), redact_value(value));
            }
            Ok(redacted)
        } else {
            Ok(HashMap::new())
        }
    }

    /// Parse a scope/key pair from a string like "scope/key"
    pub fn parse_scope_key(input: &str) -> Result<(String, String), StoreError> {
        let parts: Vec<&str> = input.splitn(2, '/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(StoreError::InvalidFormat {
                input: input.to_string(),
            });
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Check file permissions and warn if too permissive
    #[cfg(unix)]
    pub fn check_permissions(&self) -> Result<(), StoreError> {
        use std::os::unix::fs::MetadataExt;

        if !self.secrets_file.exists() {
            return Ok(());
        }

        let metadata = fs::metadata(&self.secrets_file).map_err(|e| StoreError::FileReadError {
            message: format!("Failed to read file metadata: {}", e),
        })?;

        let mode = metadata.mode();
        let permissions = mode & 0o777;

        if permissions & 0o077 != 0 {
            warn!(
                "Secrets file {} has permissive permissions: {:o}. Recommended: 600",
                self.secrets_file.display(),
                permissions
            );
        }

        Ok(())
    }

    #[cfg(not(unix))]
    pub fn check_permissions(&self) -> Result<(), StoreError> {
        Ok(())
    }
}

/// Redact a secret value for display
pub fn redact_value(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count <= 6 {
        "***".to_string()
    } else {
        let prefix: String = value.chars().take(3).collect();
        format!("{}***", prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_store() -> (TempDir, SecretsStore) {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("test_secrets.json");
        let store = SecretsStore::new(store_path);
        (temp_dir, store)
    }

    #[test]
    fn test_empty_store() {
        let (_temp_dir, store) = setup_test_store();
        let secrets = store.load().unwrap();
        assert!(secrets.is_empty());
    }

    #[test]
    fn test_set_and_get() {
        let (_temp_dir, store) = setup_test_store();

        store.set("database", "password", "secret123").unwrap();
        let value = store.get("database", "password").unwrap();
        assert_eq!(value, "secret123");
    }

    #[test]
    fn test_delete() {
        let (_temp_dir, store) = setup_test_store();

        store.set("api", "key", "abc123").unwrap();
        assert!(store.get("api", "key").is_ok());

        store.delete("api", "key").unwrap();
        assert!(matches!(
            store.get("api", "key"),
            Err(StoreError::SecretNotFound { .. })
        ));
    }

    #[test]
    fn test_list_redacted() {
        let (_temp_dir, store) = setup_test_store();

        store.set("db", "password", "verysecretpassword").unwrap();
        store.set("api", "token", "short").unwrap();

        let list = store.list().unwrap();
        assert_eq!(list["db"]["password"], "ver***");
        assert_eq!(list["api"]["token"], "***");
    }

    #[test]
    fn test_parse_scope_key() {
        let (scope, key) = SecretsStore::parse_scope_key("database/password").unwrap();
        assert_eq!(scope, "database");
        assert_eq!(key, "password");

        assert!(SecretsStore::parse_scope_key("invalid").is_err());
        assert!(SecretsStore::parse_scope_key("/key").is_err());
        assert!(SecretsStore::parse_scope_key("scope/").is_err());
    }

    #[test]
    fn test_atomic_writes() {
        let (_temp_dir, store) = setup_test_store();

        // Set initial values
        store.set("test", "key1", "value1").unwrap();
        store.set("test", "key2", "value2").unwrap();

        // Verify they exist
        assert_eq!(store.get("test", "key1").unwrap(), "value1");
        assert_eq!(store.get("test", "key2").unwrap(), "value2");

        // Update one value
        store.set("test", "key1", "updated").unwrap();

        // Both should still be accessible
        assert_eq!(store.get("test", "key1").unwrap(), "updated");
        assert_eq!(store.get("test", "key2").unwrap(), "value2");
    }

    #[test]
    fn test_empty_scope_removal() {
        let (_temp_dir, store) = setup_test_store();

        store.set("temp", "key", "value").unwrap();
        let secrets = store.load().unwrap();
        assert!(secrets.contains_key("temp"));

        store.delete("temp", "key").unwrap();
        let secrets = store.load().unwrap();
        assert!(!secrets.contains_key("temp"));
    }

    #[test]
    fn test_redact_value() {
        assert_eq!(redact_value("abc"), "***");
        assert_eq!(redact_value("secret"), "***");
        assert_eq!(redact_value("verylongsecret"), "ver***");
    }

    #[test]
    fn test_redact_value_multibyte() {
        // Test with emoji and non-ASCII characters
        assert_eq!(redact_value("🔑🔒secret"), "🔑🔒s***");
        assert_eq!(redact_value("密码12345"), "密码1***");
        assert_eq!(redact_value("αβγδεζ"), "***");
        assert_eq!(redact_value("🚀"), "***");
        // Ensure no panic on empty or short multibyte strings
        assert_eq!(redact_value(""), "***");
        assert_eq!(redact_value("🔑"), "***");
    }
}
