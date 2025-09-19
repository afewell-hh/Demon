use once_cell::sync::OnceCell;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Secret not found: {scope}/{key}")]
    SecretNotFound { scope: String, key: String },

    #[error("Failed to read secrets file: {path} - {message}")]
    SecretsFileError { path: String, message: String },

    #[error("Failed to parse secrets file: {message}")]
    SecretsParseError { message: String },

    #[error("Invalid secret URI format: {uri}")]
    InvalidSecretUri { uri: String },
}

pub trait SecretProvider: Send + Sync {
    fn resolve(&self, scope: &str, key: &str) -> Result<String, SecretError>;
}

pub struct EnvFileSecretProvider {
    secrets_file_path: Option<PathBuf>,
    cached_secrets: OnceCell<HashMap<String, HashMap<String, String>>>,
}

impl EnvFileSecretProvider {
    pub fn new() -> Self {
        let secrets_file_path = env::var("CONFIG_SECRETS_FILE")
            .map(PathBuf::from)
            .or_else(|_| {
                let default_path = PathBuf::from(".demon/secrets.json");
                if default_path.exists() {
                    Ok(default_path)
                } else {
                    Err(())
                }
            })
            .ok();

        Self {
            secrets_file_path,
            cached_secrets: OnceCell::new(),
        }
    }

    pub fn with_secrets_file<P: Into<PathBuf>>(secrets_file_path: P) -> Self {
        Self {
            secrets_file_path: Some(secrets_file_path.into()),
            cached_secrets: OnceCell::new(),
        }
    }

    fn load_secrets_from_file(
        &self,
    ) -> Result<HashMap<String, HashMap<String, String>>, SecretError> {
        if let Some(ref path) = self.secrets_file_path {
            debug!("Loading secrets from file: {:?}", path);

            let content = fs::read_to_string(path).map_err(|e| SecretError::SecretsFileError {
                path: path.to_string_lossy().to_string(),
                message: e.to_string(),
            })?;

            let parsed: Value =
                serde_json::from_str(&content).map_err(|e| SecretError::SecretsParseError {
                    message: e.to_string(),
                })?;

            let mut secrets = HashMap::new();

            if let Some(obj) = parsed.as_object() {
                for (scope, scope_value) in obj {
                    if let Some(scope_obj) = scope_value.as_object() {
                        let mut scope_secrets = HashMap::new();
                        for (key, value) in scope_obj {
                            if let Some(string_value) = value.as_str() {
                                scope_secrets.insert(key.clone(), string_value.to_string());
                            } else {
                                warn!(
                                    "Non-string value found in secrets file for {}/{}: {:?}",
                                    scope, key, value
                                );
                            }
                        }
                        secrets.insert(scope.clone(), scope_secrets);
                    }
                }
            }

            Ok(secrets)
        } else {
            Ok(HashMap::new())
        }
    }

    fn get_cached_secrets(&self) -> &HashMap<String, HashMap<String, String>> {
        self.cached_secrets.get_or_init(|| {
            self.load_secrets_from_file().unwrap_or_else(|e| {
                warn!("Failed to load secrets from file: {}", e);
                HashMap::new()
            })
        })
    }
}

impl SecretProvider for EnvFileSecretProvider {
    fn resolve(&self, scope: &str, key: &str) -> Result<String, SecretError> {
        // First try environment variable: SECRET_<SCOPE>_<KEY>
        let env_var_name = format!("SECRET_{}_{}", scope.to_uppercase(), key.to_uppercase());

        if let Ok(value) = env::var(&env_var_name) {
            debug!(
                "Resolved secret {}/{} from environment variable {}",
                scope, key, env_var_name
            );
            return Ok(value);
        }

        // Fall back to secrets file
        let cached_secrets = self.get_cached_secrets();

        if let Some(scope_secrets) = cached_secrets.get(scope) {
            if let Some(value) = scope_secrets.get(key) {
                debug!("Resolved secret {}/{} from secrets file", scope, key);
                return Ok(value.clone());
            }
        }

        Err(SecretError::SecretNotFound {
            scope: scope.to_string(),
            key: key.to_string(),
        })
    }
}

impl Default for EnvFileSecretProvider {
    fn default() -> Self {
        Self::new()
    }
}

pub fn resolve_secrets_in_config<P: SecretProvider + ?Sized>(
    config: &mut Value,
    provider: &P,
) -> Result<(), SecretError> {
    let secret_regex = Regex::new(r"^secret://([^/]+)/(.+)$").unwrap();
    resolve_secrets_recursive(config, provider, &secret_regex)
}

fn resolve_secrets_recursive<P: SecretProvider + ?Sized>(
    value: &mut Value,
    provider: &P,
    secret_regex: &Regex,
) -> Result<(), SecretError> {
    match value {
        Value::String(s) => {
            if let Some(captures) = secret_regex.captures(s) {
                let scope = captures.get(1).unwrap().as_str();
                let key = captures.get(2).unwrap().as_str();

                let resolved_secret = provider.resolve(scope, key)?;
                *s = resolved_secret;
            }
        }
        Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                resolve_secrets_recursive(v, provider, secret_regex)?;
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                resolve_secrets_recursive(item, provider, secret_regex)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn redact_secrets_in_config(config: &mut Value) {
    let secret_regex = Regex::new(r"^[^:]*://.*").unwrap();
    redact_secrets_recursive(config, &secret_regex);
}

fn redact_secrets_recursive(value: &mut Value, secret_regex: &Regex) {
    match value {
        Value::String(s) => {
            if secret_regex.is_match(s) || s.len() > 20 {
                *s = "***".to_string();
            }
        }
        Value::Object(obj) => {
            for (key, v) in obj.iter_mut() {
                if key.to_lowercase().contains("password")
                    || key.to_lowercase().contains("secret")
                    || key.to_lowercase().contains("token")
                    || key.to_lowercase().contains("key")
                {
                    if let Value::String(s) = v {
                        *s = "***".to_string();
                    }
                } else {
                    redact_secrets_recursive(v, secret_regex);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_secrets_recursive(item, secret_regex);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_env_secret_resolution() {
        env::set_var("SECRET_TEST_PASSWORD", "env_secret_value");

        let provider = EnvFileSecretProvider::new();
        let result = provider.resolve("test", "password");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "env_secret_value");

        env::remove_var("SECRET_TEST_PASSWORD");
    }

    #[test]
    fn test_file_secret_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let secrets_file = temp_dir.path().join("secrets.json");

        let secrets_content = json!({
            "database": {
                "password": "file_secret_value",
                "username": "admin"
            }
        });

        fs::write(
            &secrets_file,
            serde_json::to_string(&secrets_content).unwrap(),
        )
        .unwrap();

        let provider = EnvFileSecretProvider::with_secrets_file(&secrets_file);

        let result = provider.resolve("database", "password");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "file_secret_value");

        let result = provider.resolve("database", "username");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "admin");
    }

    #[test]
    fn test_secret_not_found() {
        let provider = EnvFileSecretProvider::new();
        let result = provider.resolve("nonexistent", "secret");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecretError::SecretNotFound { .. }
        ));
    }

    #[test]
    fn test_resolve_secrets_in_config() {
        env::set_var("SECRET_DB_PASSWORD", "resolved_password");

        let provider = EnvFileSecretProvider::new();
        let mut config = json!({
            "database": {
                "host": "localhost",
                "password": "secret://db/password",
                "port": 5432
            },
            "api_key": "secret://api/key"
        });

        let result = resolve_secrets_in_config(&mut config, &provider);
        assert!(result.is_err()); // api/key secret doesn't exist

        env::set_var("SECRET_API_KEY", "resolved_api_key");
        let result = resolve_secrets_in_config(&mut config, &provider);
        assert!(result.is_ok());

        assert_eq!(config["database"]["password"], "resolved_password");
        assert_eq!(config["api_key"], "resolved_api_key");

        env::remove_var("SECRET_DB_PASSWORD");
        env::remove_var("SECRET_API_KEY");
    }

    #[test]
    fn test_redact_secrets_in_config() {
        let mut config = json!({
            "database": {
                "host": "localhost",
                "password": "actual_password",
                "port": 5432
            },
            "api_key": "very_long_secret_key_that_should_be_redacted",
            "normal_field": "short"
        });

        redact_secrets_in_config(&mut config);

        assert_eq!(config["database"]["password"], "***");
        assert_eq!(config["api_key"], "***");
        assert_eq!(config["normal_field"], "short");
        assert_eq!(config["database"]["host"], "localhost");
    }
}
