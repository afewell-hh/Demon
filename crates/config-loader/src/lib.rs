use anyhow::Result;
use jsonschema::{Draft, JSONSchema};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, instrument};

pub mod secrets;
pub use secrets::{EnvFileSecretProvider, SecretError, SecretProvider};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Schema not found for capsule: {capsule}")]
    SchemaNotFound { capsule: String },

    #[error("Config file not found: {path}")]
    ConfigFileNotFound { path: String },

    #[error("Schema compilation failed: {message}")]
    SchemaCompilationFailed { message: String },

    #[error("Config validation failed")]
    ValidationFailed { errors: Vec<ValidationError> },

    #[error("JSON parsing failed: {message}")]
    JsonParsingFailed { message: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("Secret resolution failed: {error}")]
    SecretResolutionFailed { error: SecretError },
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub json_pointer: String,
    pub message: String,
    pub schema_path: String,
}

pub struct ConfigManager {
    contracts_dir: PathBuf,
    config_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Self {
        let contracts_dir =
            Self::find_contracts_dir().unwrap_or_else(|| PathBuf::from("contracts"));
        let config_dir = Self::find_config_dir();

        Self {
            contracts_dir,
            config_dir,
        }
    }

    pub fn with_dirs(contracts_dir: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            contracts_dir,
            config_dir,
        }
    }

    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    fn find_contracts_dir() -> Option<PathBuf> {
        // Check environment variable first
        if let Ok(contracts_dir) = std::env::var("CONTRACTS_DIR") {
            let path = PathBuf::from(contracts_dir);
            if path.is_dir() {
                return Some(path);
            }
        }

        // Fall back to searching up the directory tree
        let mut current = std::env::current_dir().ok()?;
        loop {
            let contracts_path = current.join("contracts");
            if contracts_path.is_dir() {
                return Some(contracts_path);
            }
            if !current.pop() {
                break;
            }
        }
        None
    }

    fn find_config_dir() -> PathBuf {
        if let Ok(config_dir) = std::env::var("CONFIG_DIR") {
            PathBuf::from(config_dir)
        } else {
            PathBuf::from(".demon/config")
        }
    }

    #[instrument(skip(self))]
    pub fn load<T: DeserializeOwned>(&self, link_name: &str) -> Result<T, ConfigError> {
        self.load_with_secrets(link_name, &EnvFileSecretProvider::new())
    }

    #[instrument(skip(self, provider))]
    pub fn load_with_secrets<T: DeserializeOwned, P: SecretProvider + ?Sized>(
        &self,
        link_name: &str,
        provider: &P,
    ) -> Result<T, ConfigError> {
        debug!("Loading config for link: {}", link_name);

        // Load and validate against schema
        let mut config_value = self.load_config_file(link_name)?;

        // Resolve secrets before validation
        secrets::resolve_secrets_in_config(&mut config_value, provider)
            .map_err(|e| ConfigError::SecretResolutionFailed { error: e })?;

        self.validate_config(link_name, &config_value)?;

        // Deserialize to the target type
        serde_json::from_value(config_value).map_err(|e| ConfigError::JsonParsingFailed {
            message: e.to_string(),
        })
    }

    #[instrument(skip(self))]
    pub fn validate_config_file(
        &self,
        capsule: &str,
        config_path: &Path,
    ) -> Result<(), ConfigError> {
        self.validate_config_file_with_secrets(capsule, config_path, &EnvFileSecretProvider::new())
    }

    #[instrument(skip(self, provider))]
    pub fn validate_config_file_with_secrets<P: SecretProvider + ?Sized>(
        &self,
        capsule: &str,
        config_path: &Path,
        provider: &P,
    ) -> Result<(), ConfigError> {
        debug!(
            "Validating config file: {:?} for capsule: {}",
            config_path, capsule
        );

        if !config_path.exists() {
            return Err(ConfigError::ConfigFileNotFound {
                path: config_path.to_string_lossy().to_string(),
            });
        }

        let config_content = fs::read_to_string(config_path).map_err(|e| ConfigError::IoError {
            message: format!("Failed to read config file: {}", e),
        })?;

        let mut config_value: Value =
            serde_json::from_str(&config_content).map_err(|e| ConfigError::JsonParsingFailed {
                message: e.to_string(),
            })?;

        // Resolve secrets before validation
        secrets::resolve_secrets_in_config(&mut config_value, provider)
            .map_err(|e| ConfigError::SecretResolutionFailed { error: e })?;

        self.validate_config(capsule, &config_value)
    }

    #[instrument(skip(self))]
    pub fn validate_config_value(
        &self,
        capsule: &str,
        config_value: &Value,
    ) -> Result<(), ConfigError> {
        self.validate_config_value_with_secrets(
            capsule,
            config_value,
            &EnvFileSecretProvider::new(),
        )
    }

    #[instrument(skip(self, provider))]
    pub fn validate_config_value_with_secrets<P: SecretProvider + ?Sized>(
        &self,
        capsule: &str,
        config_value: &Value,
        provider: &P,
    ) -> Result<(), ConfigError> {
        let mut config_value = config_value.clone();

        // Resolve secrets before validation
        secrets::resolve_secrets_in_config(&mut config_value, provider)
            .map_err(|e| ConfigError::SecretResolutionFailed { error: e })?;

        self.validate_config(capsule, &config_value)
    }

    fn load_config_file(&self, link_name: &str) -> Result<Value, ConfigError> {
        let config_path = self.config_dir.join(format!("{}.json", link_name));

        debug!("Loading config from: {:?}", config_path);

        if !config_path.exists() {
            debug!("Config file not found, loading defaults from schema");
            return self.load_default_config(link_name);
        }

        let content = fs::read_to_string(&config_path).map_err(|e| ConfigError::IoError {
            message: format!("Failed to read config file: {}", e),
        })?;

        serde_json::from_str(&content).map_err(|e| ConfigError::JsonParsingFailed {
            message: e.to_string(),
        })
    }

    fn load_default_config(&self, link_name: &str) -> Result<Value, ConfigError> {
        // Load the schema to extract defaults
        let schema_path = self
            .contracts_dir
            .join("config")
            .join(format!("{}-config.v1.json", link_name));

        if !schema_path.exists() {
            return Err(ConfigError::SchemaNotFound {
                capsule: link_name.to_string(),
            });
        }

        let schema_content =
            fs::read_to_string(&schema_path).map_err(|e| ConfigError::IoError {
                message: format!("Failed to read schema file: {}", e),
            })?;

        let schema_value: Value =
            serde_json::from_str(&schema_content).map_err(|e| ConfigError::JsonParsingFailed {
                message: e.to_string(),
            })?;

        // Extract defaults from schema properties
        let mut default_config = serde_json::Map::new();

        if let Some(properties) = schema_value.get("properties").and_then(|p| p.as_object()) {
            for (key, property) in properties {
                if let Some(default_value) = property.get("default") {
                    default_config.insert(key.clone(), default_value.clone());
                }
            }
        }

        let default_config_value = Value::Object(default_config);

        // Validate that the default config is valid according to the schema
        // This ensures the schema defaults work correctly
        self.validate_config(link_name, &default_config_value)?;

        debug!("Loaded default config: {}", default_config_value);
        Ok(default_config_value)
    }

    fn validate_config(&self, capsule: &str, config: &Value) -> Result<(), ConfigError> {
        let schema = self.get_compiled_schema(capsule)?;
        let validation_result = schema.validate(config);

        if let Err(errors) = validation_result {
            let validation_errors: Vec<ValidationError> = errors
                .map(|error| ValidationError {
                    json_pointer: error.instance_path.to_string(),
                    message: error.to_string(),
                    schema_path: error.schema_path.to_string(),
                })
                .collect();

            return Err(ConfigError::ValidationFailed {
                errors: validation_errors,
            });
        }

        Ok(())
    }

    fn get_compiled_schema(&self, capsule: &str) -> Result<JSONSchema, ConfigError> {
        // Load and compile schema (simplified without caching for now)
        let schema_path = self
            .contracts_dir
            .join("config")
            .join(format!("{}-config.v1.json", capsule));

        if !schema_path.exists() {
            return Err(ConfigError::SchemaNotFound {
                capsule: capsule.to_string(),
            });
        }

        let schema_content =
            fs::read_to_string(&schema_path).map_err(|e| ConfigError::IoError {
                message: format!("Failed to read schema file: {}", e),
            })?;

        let schema_value: Value =
            serde_json::from_str(&schema_content).map_err(|e| ConfigError::JsonParsingFailed {
                message: e.to_string(),
            })?;

        let compiled_schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| ConfigError::SchemaCompilationFailed {
                message: e.to_string(),
            })?;

        Ok(compiled_schema)
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn setup_test_env() -> (TempDir, ConfigManager) {
        let temp_dir = TempDir::new().unwrap();
        let contracts_dir = temp_dir.path().join("contracts");
        let config_dir = temp_dir.path().join("config");

        fs::create_dir_all(contracts_dir.join("config")).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        // Create test schema
        let schema_content = r#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "messagePrefix": { "type": "string", "default": "" },
                "enableTrim": { "type": "boolean", "default": true },
                "maxMessageLength": { "type": "integer", "minimum": 1, "default": 1000 },
                "outputFormat": { "type": "string", "enum": ["plain", "json", "structured"], "default": "plain" }
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
    fn test_load_valid_config() {
        let (_temp_dir, manager) = setup_test_env();

        let config_content = r#"{
            "messagePrefix": "Test: ",
            "enableTrim": true,
            "maxMessageLength": 500,
            "outputFormat": "plain"
        }"#;

        fs::write(manager.config_dir.join("echo.json"), config_content).unwrap();

        let config: EchoConfig = manager.load("echo").unwrap();
        assert_eq!(config.message_prefix, "Test: ");
        assert!(config.enable_trim);
        assert_eq!(config.max_message_length, Some(500));
        assert_eq!(config.output_format, Some("plain".to_string()));
    }

    #[test]
    fn test_load_invalid_config_missing_required() {
        let (_temp_dir, manager) = setup_test_env();

        let config_content = r#"{
            "messagePrefix": "Test: "
        }"#;

        fs::write(manager.config_dir.join("echo.json"), config_content).unwrap();

        let result: Result<EchoConfig, ConfigError> = manager.load("echo");
        assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));
    }

    #[test]
    fn test_validate_config_file() {
        let (_temp_dir, manager) = setup_test_env();

        let valid_config = r#"{
            "messagePrefix": "Test: ",
            "enableTrim": true
        }"#;

        let config_path = manager.config_dir.join("test_config.json");
        fs::write(&config_path, valid_config).unwrap();

        assert!(manager.validate_config_file("echo", &config_path).is_ok());

        let invalid_config = r#"{
            "messagePrefix": "Test: ",
            "enableTrim": "not_a_boolean"
        }"#;

        fs::write(&config_path, invalid_config).unwrap();
        assert!(manager.validate_config_file("echo", &config_path).is_err());
    }

    #[test]
    fn test_load_missing_config_uses_defaults() {
        let (_temp_dir, manager) = setup_test_env();

        // Don't create a config file - should load defaults from schema
        let config: EchoConfig = manager.load("echo").unwrap();

        // Should get the defaults from the schema
        assert_eq!(config.message_prefix, "");
        assert!(config.enable_trim);
        assert_eq!(config.max_message_length, Some(1000));
        assert_eq!(config.output_format, Some("plain".to_string()));
    }

    #[test]
    fn test_validate_config_file_missing_still_fails() {
        let (_temp_dir, manager) = setup_test_env();

        let missing_path = manager.config_dir.join("missing.json");
        let result = manager.validate_config_file("echo", &missing_path);

        // Explicit file validation should still fail for missing files
        assert!(matches!(
            result,
            Err(ConfigError::ConfigFileNotFound { .. })
        ));
    }
}
