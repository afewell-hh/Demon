//! JetStream KV client for contract metadata storage
//!
//! Provides CRUD operations for contract schema bundles stored in JetStream KV.
//! Key layout: contracts.meta.<name>.<version>

use anyhow::{Context, Result};
use async_nats::jetstream::{self, kv::Store};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Contract metadata stored in KV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Full contract bundle including schemas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractBundle {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "jsonSchema")]
    pub json_schema: Option<String>,
    #[serde(rename = "witPath")]
    pub wit_path: Option<String>,
    #[serde(rename = "descriptorPath")]
    pub descriptor_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

/// JetStream KV client for contract storage
#[derive(Clone)]
pub struct KvClient {
    kv_store: Store,
}

impl KvClient {
    /// Create a new KV client connected to JetStream
    pub async fn new() -> Result<Self> {
        let nats_url =
            std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
        let bucket_name =
            std::env::var("REGISTRY_KV_BUCKET").unwrap_or_else(|_| "contracts".to_string());

        Self::with_connection(&nats_url, &bucket_name).await
    }

    /// Create a KV client using the provided connection settings.
    async fn with_connection(nats_url: &str, bucket_name: &str) -> Result<Self> {
        info!(
            "Connecting to NATS at {} for KV operations (bucket: {})",
            nats_url, bucket_name
        );

        let client = if let Ok(creds_path) = std::env::var("NATS_CREDS_PATH") {
            info!("Using credentials file: {}", creds_path);
            async_nats::ConnectOptions::new()
                .credentials_file(&creds_path)
                .await?
                .connect(nats_url)
                .await?
        } else {
            warn!("No NATS credentials provided, connecting without auth");
            async_nats::connect(nats_url).await?
        };

        let jetstream = jetstream::new(client);

        // Get or create KV bucket for contract metadata
        let kv_store = match jetstream.get_key_value(bucket_name).await {
            Ok(store) => {
                info!("Using existing KV bucket: {}", bucket_name);
                store
            }
            Err(_) => {
                info!("Creating new KV bucket: {}", bucket_name);
                let config = jetstream::kv::Config {
                    bucket: bucket_name.to_string(),
                    description: "Contract schema registry metadata".to_string(),
                    ..Default::default()
                };
                jetstream.create_key_value(config).await?
            }
        };

        Ok(Self { kv_store })
    }

    /// List all contract metadata entries
    pub async fn list_contracts(&self) -> Result<Vec<ContractMetadata>> {
        debug!("Listing all contracts from KV");

        let mut contracts = Vec::new();

        // Get all keys with "meta." prefix
        let mut keys = self.kv_store.keys().await?.boxed();

        while let Some(key_result) = keys.next().await {
            match key_result {
                Ok(key) => {
                    if key.starts_with("meta.") {
                        if let Ok(Some(bytes)) = self.kv_store.get(&key).await {
                            match serde_json::from_slice::<ContractMetadata>(&bytes) {
                                Ok(metadata) => contracts.push(metadata),
                                Err(e) => {
                                    warn!("Failed to parse metadata for key {}: {}", key, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error reading key from KV: {}", e);
                }
            }
        }

        info!("Retrieved {} contracts from KV", contracts.len());
        Ok(contracts)
    }

    /// Get a specific contract bundle by name and version
    pub async fn get_contract(&self, name: &str, version: &str) -> Result<Option<ContractBundle>> {
        let key = format!("meta.{}.{}", name, version);
        debug!("Fetching contract from KV: {}", key);

        match self.kv_store.get(&key).await? {
            Some(bytes) => {
                let bundle = serde_json::from_slice::<ContractBundle>(&bytes)
                    .with_context(|| format!("Failed to parse contract bundle for {}", key))?;
                info!("Retrieved contract: {} v{}", name, version);
                Ok(Some(bundle))
            }
            None => {
                debug!("Contract not found: {} v{}", name, version);
                Ok(None)
            }
        }
    }

    /// Store a contract bundle in KV
    pub async fn put_contract(&self, bundle: &ContractBundle) -> Result<()> {
        let key = format!("meta.{}.{}", bundle.name, bundle.version);
        debug!("Storing contract in KV: {}", key);

        let value = serde_json::to_vec(bundle)
            .with_context(|| format!("Failed to serialize contract bundle for {}", key))?;

        self.kv_store
            .put(&key, value.into())
            .await
            .with_context(|| format!("Failed to store contract in KV: {}", key))?;

        info!("Stored contract: {} v{}", bundle.name, bundle.version);
        Ok(())
    }

    /// Delete a contract from KV
    pub async fn delete_contract(&self, name: &str, version: &str) -> Result<()> {
        let key = format!("meta.{}.{}", name, version);
        debug!("Deleting contract from KV: {}", key);

        self.kv_store
            .delete(&key)
            .await
            .with_context(|| format!("Failed to delete contract from KV: {}", key))?;

        info!("Deleted contract: {} v{}", name, version);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_metadata_serialization() {
        let metadata = ContractMetadata {
            name: "test-contract".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test contract".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("test-contract"));
        assert!(json.contains("createdAt"));

        let deserialized: ContractMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-contract");
        assert_eq!(deserialized.version, "1.0.0");
    }

    #[test]
    fn test_contract_bundle_serialization() {
        let bundle = ContractBundle {
            name: "test-contract".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test bundle".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            json_schema: Some("{}".to_string()),
            wit_path: Some("/path/to/schema.wit".to_string()),
            descriptor_path: Some("/path/to/descriptor.json".to_string()),
            digest: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("jsonSchema"));
        assert!(json.contains("witPath"));
        assert!(json.contains("descriptorPath"));
        assert!(json.contains("digest"));

        let deserialized: ContractBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-contract");
        assert_eq!(deserialized.digest, Some("abc123".to_string()));
    }
}
