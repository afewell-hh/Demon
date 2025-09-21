use crate::secrets::{SecretError, SecretProvider};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum VaultHttpError {
    #[error("Vault HTTP request failed: {message}")]
    RequestFailed { message: String },

    #[error("Vault authentication failed: {message}")]
    AuthFailed { message: String },

    #[error("Invalid Vault response format: {message}")]
    InvalidResponse { message: String },

    #[error("Vault configuration error: {message}")]
    ConfigError { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultKV2Response {
    data: VaultKV2Data,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultKV2Data {
    data: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
struct VaultKV2WriteRequest {
    data: HashMap<String, String>,
}

pub struct VaultHttpSecretProvider {
    client: Client,
    vault_addr: String,
    vault_token: String,
    vault_namespace: Option<String>,
    max_retries: u32,
}

impl VaultHttpSecretProvider {
    pub fn new(
        vault_addr: Option<String>,
        vault_token: Option<String>,
        vault_namespace: Option<String>,
    ) -> Result<Self, VaultHttpError> {
        let addr = vault_addr
            .or_else(|| env::var("VAULT_ADDR").ok())
            .unwrap_or_else(|| "http://127.0.0.1:8200".to_string());

        if !addr.starts_with("http://") && !addr.starts_with("https://") {
            return Err(VaultHttpError::ConfigError {
                message: format!(
                    "Invalid VAULT_ADDR format: {}. Must start with http:// or https://",
                    addr
                ),
            });
        }

        let token = vault_token
            .or_else(|| env::var("VAULT_TOKEN").ok())
            .ok_or_else(|| VaultHttpError::ConfigError {
                message: "VAULT_TOKEN is required for HTTP provider".to_string(),
            })?;

        let namespace = vault_namespace.or_else(|| env::var("VAULT_NAMESPACE").ok());

        let max_retries = env::var("VAULT_RETRY_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(3);

        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10));

        // Handle TLS configuration
        if let Ok(ca_cert_path) = env::var("VAULT_CA_CERT") {
            debug!("Loading CA certificate from: {}", ca_cert_path);
            let cert_contents =
                std::fs::read(&ca_cert_path).map_err(|e| VaultHttpError::ConfigError {
                    message: format!("Failed to read CA certificate from {}: {}", ca_cert_path, e),
                })?;

            let cert = reqwest::Certificate::from_pem(&cert_contents).map_err(|e| {
                VaultHttpError::ConfigError {
                    message: format!("Invalid CA certificate format: {}", e),
                }
            })?;

            client_builder = client_builder.add_root_certificate(cert);
        }

        // Handle skip verify for development
        if env::var("VAULT_SKIP_VERIFY").unwrap_or_default() == "true" {
            warn!("VAULT_SKIP_VERIFY is set to true - TLS verification disabled (INSECURE)");
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder
            .build()
            .map_err(|e| VaultHttpError::ConfigError {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(Self {
            client,
            vault_addr: addr.trim_end_matches('/').to_string(),
            vault_token: token,
            vault_namespace: namespace,
            max_retries,
        })
    }

    pub fn from_env() -> Result<Self, VaultHttpError> {
        Self::new(None, None, None)
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Vault-Token",
            HeaderValue::from_str(&self.vault_token).unwrap(),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(ref namespace) = self.vault_namespace {
            headers.insert(
                "X-Vault-Namespace",
                HeaderValue::from_str(namespace).unwrap(),
            );
        }

        headers
    }

    fn retry_with_backoff<F, T>(&self, mut operation: F) -> Result<T, VaultHttpError>
    where
        F: FnMut() -> Result<T, VaultHttpError>,
    {
        let mut last_error = None;
        let mut backoff_ms = 100;

        for attempt in 0..self.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Don't retry on auth failures or client errors (including 404)
                    match &e {
                        VaultHttpError::AuthFailed { .. } => return Err(e),
                        VaultHttpError::RequestFailed { message }
                            if message.contains("not found") =>
                        {
                            return Err(e)
                        }
                        _ => {}
                    }

                    last_error = Some(e);

                    if attempt < self.max_retries - 1 {
                        debug!(
                            "Attempt {} failed, retrying in {}ms",
                            attempt + 1,
                            backoff_ms
                        );
                        std::thread::sleep(Duration::from_millis(backoff_ms));
                        backoff_ms *= 2; // Exponential backoff
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| VaultHttpError::RequestFailed {
            message: "All retry attempts exhausted".to_string(),
        }))
    }

    fn kv_path(&self, scope: &str, key: &str) -> String {
        format!("{}/v1/secret/data/{}/{}", self.vault_addr, scope, key)
    }

    pub fn resolve_secret(&self, scope: &str, key: &str) -> Result<String, SecretError> {
        let url = self.kv_path(scope, key);
        debug!("Resolving secret from Vault: {}/{}", scope, key);

        let result = self.retry_with_backoff(|| {
            let response = self
                .client
                .get(&url)
                .headers(self.build_headers())
                .send()
                .map_err(|e| VaultHttpError::RequestFailed {
                    message: format!("HTTP request failed: {}", e),
                })?;

            match response.status().as_u16() {
                200 => {
                    let vault_response: VaultKV2Response =
                        response
                            .json()
                            .map_err(|e| VaultHttpError::InvalidResponse {
                                message: format!("Failed to parse response: {}", e),
                            })?;

                    if let Some(ref data) = vault_response.data.data {
                        if let Some(value) = data.get(key) {
                            return Ok(value.clone());
                        }
                    }

                    Err(VaultHttpError::InvalidResponse {
                        message: format!("Key '{}' not found in Vault response", key),
                    })
                }
                401 | 403 => Err(VaultHttpError::AuthFailed {
                    message: format!("Authentication failed: {}", response.status()),
                }),
                404 => Err(VaultHttpError::RequestFailed {
                    message: "Secret not found".to_string(),
                }),
                status if status >= 500 => Err(VaultHttpError::RequestFailed {
                    message: format!("Server error: {}", response.status()),
                }),
                _ => Err(VaultHttpError::RequestFailed {
                    message: format!("Unexpected status: {}", response.status()),
                }),
            }
        });

        match result {
            Ok(value) => Ok(value),
            Err(VaultHttpError::RequestFailed { message }) if message.contains("not found") => {
                Err(SecretError::SecretNotFound {
                    scope: scope.to_string(),
                    key: key.to_string(),
                })
            }
            Err(VaultHttpError::AuthFailed { .. }) => Err(SecretError::SecretsFileError {
                path: url,
                message: "Vault authentication failed".to_string(),
            }),
            Err(e) => Err(SecretError::SecretsFileError {
                path: url,
                message: e.to_string(),
            }),
        }
    }

    pub fn put(&self, scope: &str, key: &str, value: &str) -> Result<(), String> {
        let url = self.kv_path(scope, key);
        debug!("Storing secret to Vault: {}/{}", scope, key);

        let mut data = HashMap::new();
        data.insert(key.to_string(), value.to_string());

        let request_body = VaultKV2WriteRequest { data };

        self.retry_with_backoff(|| {
            let response = self
                .client
                .post(&url)
                .headers(self.build_headers())
                .json(&request_body)
                .send()
                .map_err(|e| VaultHttpError::RequestFailed {
                    message: format!("HTTP request failed: {}", e),
                })?;

            match response.status().as_u16() {
                200 | 204 => Ok(()),
                401 | 403 => Err(VaultHttpError::AuthFailed {
                    message: format!("Authentication failed: {}", response.status()),
                }),
                status if status >= 500 => Err(VaultHttpError::RequestFailed {
                    message: format!("Server error: {}", response.status()),
                }),
                _ => Err(VaultHttpError::RequestFailed {
                    message: format!("Failed to store secret: {}", response.status()),
                }),
            }
        })
        .map_err(|e| e.to_string())
    }

    pub fn delete(&self, scope: &str, key: &str) -> Result<(), String> {
        let url = format!("{}/v1/secret/metadata/{}/{}", self.vault_addr, scope, key);
        debug!("Deleting secret from Vault: {}/{}", scope, key);

        self.retry_with_backoff(|| {
            let response = self
                .client
                .delete(&url)
                .headers(self.build_headers())
                .send()
                .map_err(|e| VaultHttpError::RequestFailed {
                    message: format!("HTTP request failed: {}", e),
                })?;

            match response.status().as_u16() {
                200 | 204 => Ok(()),
                401 | 403 => Err(VaultHttpError::AuthFailed {
                    message: format!("Authentication failed: {}", response.status()),
                }),
                404 => Err(VaultHttpError::RequestFailed {
                    message: "Secret not found".to_string(),
                }),
                status if status >= 500 => Err(VaultHttpError::RequestFailed {
                    message: format!("Server error: {}", response.status()),
                }),
                _ => Err(VaultHttpError::RequestFailed {
                    message: format!("Failed to delete secret: {}", response.status()),
                }),
            }
        })
        .map_err(|e| e.to_string())
    }

    pub fn list(
        &self,
        _scope: Option<&str>,
    ) -> Result<HashMap<String, HashMap<String, String>>, String> {
        // For now, return an error as listing is complex in Vault KV v2
        // This would require LIST operations which have different semantics
        Err("List operation not implemented for HTTP provider".to_string())
    }
}

impl SecretProvider for VaultHttpSecretProvider {
    fn resolve(&self, scope: &str, key: &str) -> Result<String, SecretError> {
        self.resolve_secret(scope, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_http_provider_creation() {
        // Test with missing token should fail
        std::env::remove_var("VAULT_TOKEN");
        let result = VaultHttpSecretProvider::from_env();
        assert!(result.is_err());

        // Test with token should succeed
        std::env::set_var("VAULT_TOKEN", "test-token");
        let provider = VaultHttpSecretProvider::from_env().unwrap();
        assert_eq!(provider.vault_addr, "http://127.0.0.1:8200");
        std::env::remove_var("VAULT_TOKEN");
    }

    #[test]
    fn test_invalid_vault_addr() {
        let result = VaultHttpSecretProvider::new(
            Some("invalid://addr".to_string()),
            Some("token".to_string()),
            None,
        );
        assert!(result.is_err());
    }
}
