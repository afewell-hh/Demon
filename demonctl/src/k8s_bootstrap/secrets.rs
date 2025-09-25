use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use config_loader::provider_factory::VaultStubProvider;
use config_loader::secrets::SecretProvider;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use tracing::{debug, info, warn};

use super::{RegistryConfig, SecretsConfig, VaultConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMaterial {
    pub data: HashMap<String, String>,
    pub provider_used: String,
}

impl SecretMaterial {
    pub fn new(provider: &str) -> Self {
        Self {
            data: HashMap::new(),
            provider_used: provider.to_string(),
        }
    }

    pub fn add_secret(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    pub fn merge(&mut self, other: SecretMaterial) {
        for (key, value) in other.data {
            self.data.insert(key, value);
        }
        if !other.provider_used.is_empty() && !self.provider_used.contains(&other.provider_used) {
            self.provider_used
                .push_str(&format!("+{}", other.provider_used));
        }
    }
}

pub fn collect_secrets(config: &SecretsConfig, dry_run: bool) -> Result<SecretMaterial> {
    let mut result = SecretMaterial::new("");
    let mut providers_used = Vec::new();

    // Collect from env provider
    if let Some(env_map) = &config.env {
        debug!("Collecting secrets from env provider");
        let mut env_secrets = SecretMaterial::new("env");

        for (key, env_var) in env_map {
            if dry_run {
                // In dry-run, use placeholder value for security
                info!("Would fetch secret '{}' from environment variable", key);
                env_secrets.add_secret(key.clone(), "REDACTED".to_string());
            } else {
                let value = env::var(env_var).with_context(|| {
                    format!(
                        "Environment variable '{}' not found for secret '{}'",
                        env_var, key
                    )
                })?;
                env_secrets.add_secret(key.clone(), value);
            }
        }

        providers_used.push(format!("env ({} keys)", env_map.len()));
        result.merge(env_secrets);
    }

    // Collect from vault provider
    if config.provider == "vault" || config.vault.is_some() {
        debug!("Collecting secrets from vault provider");

        if let Some(vault_config) = &config.vault {
            if dry_run {
                // In dry-run, validate configuration but don't fetch
                validate_vault_config(vault_config)?;
                info!("Vault provider configured at path: {}", vault_config.path);
                providers_used.push("vault (configured)".to_string());
            } else {
                let vault_secrets = fetch_from_vault(vault_config)?;
                providers_used.push(format!("vault ({} keys)", vault_secrets.data.len()));
                result.merge(vault_secrets);
            }
        } else if config.provider == "vault" {
            return Err(anyhow::anyhow!(
                "Vault provider specified but no vault configuration provided"
            ));
        }
    }

    result.provider_used = providers_used.join(", ");
    Ok(result)
}

fn validate_vault_config(vault_config: &VaultConfig) -> Result<()> {
    // Check for vault address
    let vault_addr = vault_config
        .address
        .clone()
        .or_else(|| env::var("VAULT_ADDR").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Vault address not specified in config or VAULT_ADDR environment variable"
            )
        })?;

    debug!("Vault address: {}", vault_addr);

    // Check for vault token if using token auth
    if vault_config.auth_method == "token" {
        env::var("VAULT_TOKEN")
            .with_context(|| "VAULT_TOKEN environment variable required for token auth method")?;
    }

    Ok(())
}

fn fetch_from_vault(vault_config: &VaultConfig) -> Result<SecretMaterial> {
    let vault_addr = vault_config
        .address
        .clone()
        .or_else(|| env::var("VAULT_ADDR").ok());

    let vault_token = env::var("VAULT_TOKEN").ok();

    // Create vault provider
    let provider = if let Some(addr) = vault_addr {
        VaultStubProvider::new(Some(addr), vault_token)
            .map_err(|e| anyhow::anyhow!("Failed to create vault provider: {}", e))?
    } else {
        VaultStubProvider::from_env().map_err(|e| {
            anyhow::anyhow!("Failed to create vault provider from environment: {}", e)
        })?
    };

    // Parse the path to extract scope and optional key
    let parts: Vec<&str> = vault_config.path.split('/').collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Invalid vault path: {}", vault_config.path));
    }

    let scope = parts[0];
    let mut vault_secrets = SecretMaterial::new("vault");

    if parts.len() == 1 {
        // Fetch all secrets from the scope
        warn!("Fetching all secrets from vault scope '{}' - consider specifying individual keys for better security", scope);

        // For now, we'll need to list and fetch individual keys
        // In a real implementation, we'd need to extend the provider interface
        return Err(anyhow::anyhow!(
            "Fetching all secrets from a vault scope is not yet implemented. Please specify individual keys."
        ));
    } else {
        // Fetch specific keys
        for key in &parts[1..] {
            let value = provider
                .resolve(scope, key)
                .with_context(|| format!("Failed to fetch secret {}/{} from vault", scope, key))?;
            vault_secrets.add_secret(key.to_string(), value);
        }
    }

    Ok(vault_secrets)
}

pub fn render_secret_manifest(
    namespace: &str,
    secret_name: Option<&str>,
    material: &SecretMaterial,
) -> Result<String> {
    if material.data.is_empty() {
        return Ok(String::new());
    }

    let name = secret_name.unwrap_or("demon-secrets");

    // Base64 encode all secret values
    let mut encoded_data = HashMap::new();
    for (key, value) in &material.data {
        if !value.is_empty() {
            encoded_data.insert(key.clone(), BASE64.encode(value.as_bytes()));
        }
    }

    // Build the manifest using serde_yaml
    let secret_manifest = serde_yaml::to_string(&serde_json::json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": {
                "app.kubernetes.io/managed-by": "demon-bootstrapper",
                "app.kubernetes.io/component": "secrets"
            }
        },
        "type": "Opaque",
        "data": encoded_data
    }))?;

    Ok(secret_manifest)
}

pub fn create_image_pull_secrets(
    registries: &[RegistryConfig],
    namespace: &str,
    dry_run: bool,
) -> Result<Vec<String>> {
    let mut manifests = Vec::new();

    for registry in registries {
        // Collect credentials from environment variables
        let username = if dry_run {
            "[DRY_RUN]".to_string()
        } else {
            env::var(&registry.username_env).with_context(|| {
                format!(
                    "Missing environment variable '{}' for registry '{}'",
                    registry.username_env, registry.name
                )
            })?
        };

        let password = if dry_run {
            "[DRY_RUN]".to_string()
        } else {
            env::var(&registry.password_env).with_context(|| {
                format!(
                    "Missing environment variable '{}' for registry '{}'",
                    registry.password_env, registry.name
                )
            })?
        };

        // Create Docker config JSON
        let docker_config = serde_json::json!({
            "auths": {
                registry.registry.clone(): {
                    "username": username,
                    "password": password,
                    "auth": BASE64.encode(format!("{}:{}", username, password).as_bytes())
                }
            }
        });

        let docker_config_json = serde_json::to_string(&docker_config)?;

        // Create the secret manifest
        let secret_name = format!("registry-{}", registry.name);
        let secret_manifest = serde_yaml::to_string(&serde_json::json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": {
                "name": secret_name,
                "namespace": namespace,
                "labels": {
                    "app.kubernetes.io/managed-by": "demon-bootstrapper",
                    "app.kubernetes.io/component": "registry-credentials"
                }
            },
            "type": "kubernetes.io/dockerconfigjson",
            "data": {
                ".dockerconfigjson": BASE64.encode(docker_config_json.as_bytes())
            }
        }))?;

        manifests.push(secret_manifest);

        info!(
            "Created imagePullSecret '{}' for registry '{}'",
            secret_name, registry.registry
        );
    }

    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_secrets_env_provider() {
        // Set up test environment variables
        env::set_var("TEST_SECRET_1", "value1");
        env::set_var("TEST_SECRET_2", "value2");

        let config = SecretsConfig {
            provider: "env".to_string(),
            vault: None,
            env: Some(HashMap::from([
                ("secret1".to_string(), "TEST_SECRET_1".to_string()),
                ("secret2".to_string(), "TEST_SECRET_2".to_string()),
            ])),
        };

        // Test real collection
        let material = collect_secrets(&config, false).unwrap();
        assert_eq!(material.data.len(), 2);
        assert_eq!(material.data.get("secret1").unwrap(), "value1");
        assert_eq!(material.data.get("secret2").unwrap(), "value2");
        assert!(material.provider_used.contains("env"));

        // Test dry-run
        let dry_material = collect_secrets(&config, true).unwrap();
        assert_eq!(dry_material.data.len(), 2);
        assert_eq!(dry_material.data.get("secret1").unwrap(), "REDACTED");
        assert!(dry_material.provider_used.contains("env"));

        // Clean up
        env::remove_var("TEST_SECRET_1");
        env::remove_var("TEST_SECRET_2");
    }

    #[test]
    fn test_collect_secrets_missing_env_var() {
        let config = SecretsConfig {
            provider: "env".to_string(),
            vault: None,
            env: Some(HashMap::from([(
                "secret1".to_string(),
                "NONEXISTENT_VAR".to_string(),
            )])),
        };

        let result = collect_secrets(&config, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("NONEXISTENT_VAR"));
    }

    #[test]
    fn test_render_secret_manifest() {
        let mut material = SecretMaterial::new("test");
        material.add_secret("username".to_string(), "admin".to_string());
        material.add_secret("password".to_string(), "secret123".to_string());

        let manifest =
            render_secret_manifest("demon-system", Some("test-secrets"), &material).unwrap();

        // Parse the manifest to verify structure
        let parsed: serde_yaml::Value = serde_yaml::from_str(&manifest).unwrap();

        assert_eq!(parsed["apiVersion"], "v1");
        assert_eq!(parsed["kind"], "Secret");
        assert_eq!(parsed["metadata"]["name"], "test-secrets");
        assert_eq!(parsed["metadata"]["namespace"], "demon-system");
        assert_eq!(parsed["type"], "Opaque");

        // Verify base64 encoding
        let data = parsed["data"].as_mapping().unwrap();
        assert_eq!(
            data.get("username").unwrap().as_str().unwrap(),
            BASE64.encode(b"admin")
        );
        assert_eq!(
            data.get("password").unwrap().as_str().unwrap(),
            BASE64.encode(b"secret123")
        );
    }

    #[test]
    fn test_render_empty_secret_manifest() {
        let material = SecretMaterial::new("test");
        let manifest = render_secret_manifest("demon-system", None, &material).unwrap();
        assert!(manifest.is_empty());
    }

    #[test]
    fn test_validate_vault_config_missing_address() {
        env::remove_var("VAULT_ADDR");
        env::remove_var("VAULT_TOKEN");

        let config = VaultConfig {
            address: None,
            role: None,
            path: "test/path".to_string(),
            auth_method: "token".to_string(),
        };

        let result = validate_vault_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Vault address"));
    }

    #[test]
    fn test_validate_vault_config_with_env_vars() {
        env::set_var("VAULT_ADDR", "http://localhost:8200");
        env::set_var("VAULT_TOKEN", "test-token");

        let config = VaultConfig {
            address: None,
            role: None,
            path: "test/path".to_string(),
            auth_method: "token".to_string(),
        };

        let result = validate_vault_config(&config);
        assert!(result.is_ok());

        env::remove_var("VAULT_ADDR");
        env::remove_var("VAULT_TOKEN");
    }

    #[test]
    fn test_merge_secret_materials() {
        let mut material1 = SecretMaterial::new("env");
        material1.add_secret("key1".to_string(), "value1".to_string());

        let mut material2 = SecretMaterial::new("vault");
        material2.add_secret("key2".to_string(), "value2".to_string());
        material2.add_secret("key1".to_string(), "vault_value1".to_string()); // Override

        material1.merge(material2);

        assert_eq!(material1.data.len(), 2);
        assert_eq!(material1.data.get("key1").unwrap(), "vault_value1"); // Vault overrides env
        assert_eq!(material1.data.get("key2").unwrap(), "value2");
        assert!(material1.provider_used.contains("vault"));
    }
}
