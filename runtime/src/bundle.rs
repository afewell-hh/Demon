use crate::audit::{BundleAuditor, BundleSource};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    pub version: String,
    pub timestamp: String,
    pub git: GitInfo,
    pub bundle: String,
    pub bundle_sha256: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub sha: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractBundle {
    pub schemas: HashMap<String, String>,
    pub wit_definitions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub tag: String,
    pub sha256: String,
    pub timestamp: String,
    pub git_sha: String,
    pub cached_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BundleStatus {
    Loaded,
    UsingFallback,
    VerificationFailed,
    DownloadError,
    Stale,
    NotLoaded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Warning,
    Error,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAlert {
    pub severity: AlertSeverity,
    pub message: String,
    pub remediation: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleState {
    pub status: BundleStatus,
    pub metadata: Option<BundleMetadata>,
    pub alerts: Vec<BundleAlert>,
    pub last_check: String,
    pub using_fallback: bool,
}

#[derive(Clone)]
pub struct BundleLoader {
    cache_dir: PathBuf,
    state: Arc<RwLock<BundleState>>,
}

impl BundleLoader {
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            PathBuf::from(
                std::env::var("DEMON_CONTRACTS_CACHE")
                    .unwrap_or_else(|_| ".demon/contracts".to_string()),
            )
        });

        let initial_state = BundleState {
            status: BundleStatus::NotLoaded,
            metadata: None,
            alerts: vec![],
            last_check: chrono::Utc::now().to_rfc3339(),
            using_fallback: false,
        };

        Self {
            cache_dir,
            state: Arc::new(RwLock::new(initial_state)),
        }
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Get the current bundle state
    pub async fn get_state(&self) -> BundleState {
        self.state.read().await.clone()
    }

    /// Load contract bundle from cache or download if needed
    pub async fn load_bundle(&self, tag: Option<String>) -> Result<ContractBundle> {
        let tag = tag.unwrap_or_else(|| {
            std::env::var("DEMON_CONTRACTS_TAG").unwrap_or_else(|_| "contracts-latest".to_string())
        });

        info!("Loading contract bundle: {}", tag);

        // Check cache first
        let bundle_dir = self.cache_dir.join(&tag);
        if bundle_dir.exists() {
            if let Ok(bundle) = self.load_from_cache(&bundle_dir, &tag).await {
                info!("Loaded bundle from cache: {}", bundle_dir.display());
                return Ok(bundle);
            }
        }

        // Download bundle if not cached or cache load failed
        self.download_and_cache(&tag).await
    }

    async fn load_from_cache(&self, bundle_dir: &Path, tag: &str) -> Result<ContractBundle> {
        let start_time = Instant::now();
        let manifest_path = bundle_dir.join("manifest.json");
        let bundle_path = bundle_dir.join("bundle.json");

        // Load and validate manifest
        let manifest_content = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest from cache: {:?}", manifest_path))?;
        let manifest: BundleManifest =
            serde_json::from_str(&manifest_content).context("Failed to parse cached manifest")?;

        // Load bundle
        let bundle_content = fs::read_to_string(&bundle_path)
            .with_context(|| format!("Failed to read bundle from cache: {:?}", bundle_path))?;
        let bundle: ContractBundle =
            serde_json::from_str(&bundle_content).context("Failed to parse cached bundle")?;

        // Clear previous alerts
        self.clear_alerts().await;

        // Verify SHA-256 if not skipped
        if std::env::var("DEMON_SKIP_BUNDLE_VERIFICATION").is_err() {
            let actual_sha = sha256_hex(bundle_content.as_bytes());
            if actual_sha != manifest.bundle_sha256 {
                // Emit verification failed audit event
                let event = BundleAuditor::verification_failed(
                    tag.to_string(),
                    manifest.bundle_sha256.clone(),
                    actual_sha.clone(),
                    "Check network connection and retry bundle download, or set DEMON_SKIP_BUNDLE_VERIFICATION=1 to bypass".to_string(),
                );
                BundleAuditor::emit_event(event);

                self.add_alert(
                    AlertSeverity::Error,
                    format!("Bundle verification failed: SHA-256 mismatch (expected {}, got {})", manifest.bundle_sha256, actual_sha),
                    "Check network connection and retry bundle download, or set DEMON_SKIP_BUNDLE_VERIFICATION=1 to bypass".to_string(),
                ).await;
                self.update_status(BundleStatus::VerificationFailed).await;
                anyhow::bail!(
                    "Bundle SHA-256 mismatch: expected {}, got {}",
                    manifest.bundle_sha256,
                    actual_sha
                );
            }
        } else {
            self.add_alert(
                AlertSeverity::Warning,
                "Bundle verification is disabled".to_string(),
                "Remove DEMON_SKIP_BUNDLE_VERIFICATION environment variable to enable verification"
                    .to_string(),
            )
            .await;
        }

        // Check if bundle is stale
        let is_stale = self.check_staleness(&manifest).await;
        if is_stale {
            // Emit stale detected audit event
            if let (Ok(bundle_time), Ok(now)) = (
                chrono::DateTime::parse_from_rfc3339(&manifest.timestamp),
                chrono::DateTime::parse_from_rfc3339(&chrono::Utc::now().to_rfc3339()),
            ) {
                let age_hours = (now - bundle_time).num_hours();
                let event = BundleAuditor::stale_detected(
                    tag.to_string(),
                    manifest.timestamp.clone(),
                    age_hours,
                    "Consider updating to a newer bundle version".to_string(),
                );
                BundleAuditor::emit_event(event);
            }

            self.add_alert(
                AlertSeverity::Warning,
                format!("Bundle may be stale (timestamp: {})", manifest.timestamp),
                "Consider updating to a newer bundle version".to_string(),
            )
            .await;
            self.update_status(BundleStatus::Stale).await;
        } else {
            self.update_status(BundleStatus::Loaded).await;
        }

        // Update metadata and state
        let metadata = BundleMetadata {
            tag: tag.to_string(),
            sha256: manifest.bundle_sha256.clone(),
            timestamp: manifest.timestamp.clone(),
            git_sha: manifest.git.sha.clone(),
            cached_at: chrono::Utc::now().to_rfc3339(),
        };

        {
            let mut state = self.state.write().await;
            state.metadata = Some(metadata);
            state.using_fallback = false;
        }

        // Emit successful load audit event
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let event = BundleAuditor::bundle_loaded(
            tag.to_string(),
            manifest.bundle_sha256,
            BundleSource::Cache,
            Some(manifest.git.sha),
            Some(manifest.timestamp),
            Some(duration_ms),
        );
        BundleAuditor::emit_event(event);

        Ok(bundle)
    }

    async fn download_and_cache(&self, tag: &str) -> Result<ContractBundle> {
        let start_time = Instant::now();
        info!("Downloading contract bundle: {}", tag);

        // Emit refresh attempt audit event
        let refresh_event = BundleAuditor::refresh_attempt(tag.to_string());
        BundleAuditor::emit_event(refresh_event);

        // Create cache directory
        let bundle_dir = self.cache_dir.join(tag);
        fs::create_dir_all(&bundle_dir)
            .with_context(|| format!("Failed to create cache directory: {:?}", bundle_dir))?;

        // Build download URLs
        let base_url = format!(
            "https://github.com/{}/{}/releases/download/{}",
            std::env::var("GH_OWNER").unwrap_or_else(|_| "afewell-hh".to_string()),
            std::env::var("GH_REPO").unwrap_or_else(|_| "demon".to_string()),
            tag
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to build HTTP client")?;

        // Download manifest
        let manifest_url = format!("{}/manifest.json", base_url);
        let manifest_response = client
            .get(&manifest_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch manifest from: {}", manifest_url))?;

        if !manifest_response.status().is_success() {
            let error_msg = format!(
                "Failed to download manifest (HTTP {}): {}",
                manifest_response.status(),
                manifest_url
            );
            let remediation = self.get_network_remediation().await;

            // Emit download failed audit event
            let event = BundleAuditor::download_failed(
                tag.to_string(),
                error_msg.clone(),
                Some(format!("HTTP {}", manifest_response.status())),
                remediation.clone(),
            );
            BundleAuditor::emit_event(event);

            self.add_alert(AlertSeverity::Error, error_msg.clone(), remediation)
                .await;
            self.update_status(BundleStatus::DownloadError).await;
            anyhow::bail!(error_msg);
        }

        let manifest_content = manifest_response
            .text()
            .await
            .context("Failed to read manifest response")?;
        let manifest: BundleManifest =
            serde_json::from_str(&manifest_content).context("Failed to parse manifest")?;

        // Download bundle
        let bundle_url = format!("{}/bundle.json", base_url);
        let bundle_response = client
            .get(&bundle_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch bundle from: {}", bundle_url))?;

        if !bundle_response.status().is_success() {
            let error_msg = format!(
                "Failed to download bundle (HTTP {}): {}",
                bundle_response.status(),
                bundle_url
            );
            let remediation = self.get_network_remediation().await;

            // Emit download failed audit event
            let event = BundleAuditor::download_failed(
                tag.to_string(),
                error_msg.clone(),
                Some(format!("HTTP {}", bundle_response.status())),
                remediation.clone(),
            );
            BundleAuditor::emit_event(event);

            self.add_alert(AlertSeverity::Error, error_msg.clone(), remediation)
                .await;
            self.update_status(BundleStatus::DownloadError).await;
            anyhow::bail!(error_msg);
        }

        let bundle_content = bundle_response
            .text()
            .await
            .context("Failed to read bundle response")?;

        // Clear previous alerts
        self.clear_alerts().await;

        // Verify SHA-256
        if std::env::var("DEMON_SKIP_BUNDLE_VERIFICATION").is_err() {
            let actual_sha = sha256_hex(bundle_content.as_bytes());
            if actual_sha != manifest.bundle_sha256 {
                // Emit verification failed audit event
                let event = BundleAuditor::verification_failed(
                    tag.to_string(),
                    manifest.bundle_sha256.clone(),
                    actual_sha.clone(),
                    "Bundle may be corrupted during download. Retry download or check network connection".to_string(),
                );
                BundleAuditor::emit_event(event);

                self.add_alert(
                    AlertSeverity::Error,
                    format!("Downloaded bundle verification failed: SHA-256 mismatch (expected {}, got {})", manifest.bundle_sha256, actual_sha),
                    "Bundle may be corrupted during download. Retry download or check network connection".to_string(),
                ).await;
                self.update_status(BundleStatus::VerificationFailed).await;
                anyhow::bail!(
                    "Bundle SHA-256 mismatch: expected {}, got {}",
                    manifest.bundle_sha256,
                    actual_sha
                );
            }
            info!("Bundle SHA-256 verified successfully");
        } else {
            self.add_alert(
                AlertSeverity::Warning,
                "Bundle verification is disabled".to_string(),
                "Remove DEMON_SKIP_BUNDLE_VERIFICATION environment variable to enable verification"
                    .to_string(),
            )
            .await;
        }

        let bundle: ContractBundle =
            serde_json::from_str(&bundle_content).context("Failed to parse bundle")?;

        // Save to cache
        let manifest_path = bundle_dir.join("manifest.json");
        let bundle_path = bundle_dir.join("bundle.json");

        fs::write(&manifest_path, &manifest_content)
            .with_context(|| format!("Failed to write manifest to cache: {:?}", manifest_path))?;
        fs::write(&bundle_path, &bundle_content)
            .with_context(|| format!("Failed to write bundle to cache: {:?}", bundle_path))?;

        info!("Bundle cached at: {}", bundle_dir.display());

        // Check if bundle is stale
        let is_stale = self.check_staleness(&manifest).await;
        if is_stale {
            // Emit stale detected audit event
            if let (Ok(bundle_time), Ok(now)) = (
                chrono::DateTime::parse_from_rfc3339(&manifest.timestamp),
                chrono::DateTime::parse_from_rfc3339(&chrono::Utc::now().to_rfc3339()),
            ) {
                let age_hours = (now - bundle_time).num_hours();
                let event = BundleAuditor::stale_detected(
                    tag.to_string(),
                    manifest.timestamp.clone(),
                    age_hours,
                    "Consider checking for a newer bundle version".to_string(),
                );
                BundleAuditor::emit_event(event);
            }

            self.add_alert(
                AlertSeverity::Warning,
                format!(
                    "Downloaded bundle may be stale (timestamp: {})",
                    manifest.timestamp
                ),
                "Consider checking for a newer bundle version".to_string(),
            )
            .await;
            self.update_status(BundleStatus::Stale).await;
        } else {
            self.update_status(BundleStatus::Loaded).await;
        }

        // Update metadata and state
        let metadata = BundleMetadata {
            tag: tag.to_string(),
            sha256: manifest.bundle_sha256.clone(),
            timestamp: manifest.timestamp.clone(),
            git_sha: manifest.git.sha.clone(),
            cached_at: chrono::Utc::now().to_rfc3339(),
        };

        {
            let mut state = self.state.write().await;
            state.metadata = Some(metadata);
            state.using_fallback = false;
        }

        // Emit successful download audit event
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let event = BundleAuditor::bundle_loaded(
            tag.to_string(),
            manifest.bundle_sha256,
            BundleSource::Download,
            Some(manifest.git.sha),
            Some(manifest.timestamp),
            Some(duration_ms),
        );
        BundleAuditor::emit_event(event);

        Ok(bundle)
    }

    /// Get current bundle state
    pub async fn state(&self) -> BundleState {
        let state = self.state.read().await;

        // Emit status check audit event for active monitoring
        if let Some(ref metadata) = state.metadata {
            let event = BundleAuditor::status_check(metadata.tag.clone());
            BundleAuditor::emit_event(event);
        }

        state.clone()
    }

    /// Get current bundle metadata if loaded (legacy compatibility)
    pub async fn metadata(&self) -> Option<BundleMetadata> {
        self.state.read().await.metadata.clone()
    }

    /// Add an alert to the bundle state
    async fn add_alert(&self, severity: AlertSeverity, message: String, remediation: String) {
        let alert = BundleAlert {
            severity,
            message,
            remediation,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let mut state = self.state.write().await;
        state.alerts.push(alert);
        state.last_check = chrono::Utc::now().to_rfc3339();
    }

    /// Clear all alerts
    async fn clear_alerts(&self) {
        let mut state = self.state.write().await;
        state.alerts.clear();
    }

    /// Update bundle status
    async fn update_status(&self, status: BundleStatus) {
        let mut state = self.state.write().await;
        state.status = status;
        state.last_check = chrono::Utc::now().to_rfc3339();
    }

    /// Check if bundle is stale (older than release or configured threshold)
    async fn check_staleness(&self, manifest: &BundleManifest) -> bool {
        // Check if bundle timestamp is older than configured threshold
        let stale_threshold_hours = std::env::var("DEMON_CONTRACTS_STALE_THRESHOLD_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<i64>()
            .unwrap_or(24);

        if let (Ok(bundle_time), Ok(now)) = (
            chrono::DateTime::parse_from_rfc3339(&manifest.timestamp),
            chrono::DateTime::parse_from_rfc3339(&chrono::Utc::now().to_rfc3339()),
        ) {
            let age_hours = (now - bundle_time).num_hours();
            age_hours > stale_threshold_hours
        } else {
            false
        }
    }

    /// Perform periodic freshness check to detect drift
    pub async fn check_for_updates(&self) -> Result<bool> {
        let state = self.state.read().await;

        // Skip check if bundle loading is disabled
        if std::env::var("DEMON_SKIP_CONTRACT_BUNDLE").is_ok() {
            return Ok(false);
        }

        // Get the current tag being used
        let current_tag = if let Some(ref metadata) = state.metadata {
            metadata.tag.clone()
        } else {
            std::env::var("DEMON_CONTRACTS_TAG").unwrap_or_else(|_| "contracts-latest".to_string())
        };

        drop(state); // Release read lock before attempting update

        // Try to get latest manifest without downloading full bundle
        match self.fetch_manifest_only(&current_tag).await {
            Ok(latest_manifest) => {
                let current_state = self.state.read().await;
                if let Some(ref current_metadata) = current_state.metadata {
                    // Compare timestamps to detect newer versions
                    if latest_manifest.timestamp != current_metadata.timestamp {
                        drop(current_state);

                        // Emit update detected audit event
                        let event = BundleAuditor::update_detected(
                            current_tag.clone(),
                            format!("Run 'demonctl contracts fetch {}' to update", current_tag),
                        );
                        BundleAuditor::emit_event(event);

                        self.add_alert(
                            AlertSeverity::Info,
                            "Newer contract bundle version detected".to_string(),
                            format!("Run 'demonctl contracts fetch {}' to update", current_tag),
                        )
                        .await;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Err(e) => {
                warn!("Failed to check for bundle updates: {}", e);
                self.add_alert(
                    AlertSeverity::Warning,
                    "Unable to check for bundle updates".to_string(),
                    self.get_network_remediation().await,
                )
                .await;
                Ok(false)
            }
        }
    }

    /// Fetch only the manifest for comparison (lighter than full bundle)
    async fn fetch_manifest_only(&self, tag: &str) -> Result<BundleManifest> {
        let base_url = format!(
            "https://github.com/{}/{}/releases/download/{}",
            std::env::var("GH_OWNER").unwrap_or_else(|_| "afewell-hh".to_string()),
            std::env::var("GH_REPO").unwrap_or_else(|_| "demon".to_string()),
            tag
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30)) // Shorter timeout for check
            .build()?;

        let manifest_url = format!("{}/manifest.json", base_url);
        let response = client.get(&manifest_url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP {}: {}", response.status(), manifest_url);
        }

        let content = response.text().await?;
        let manifest: BundleManifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    /// Get context-aware network remediation advice
    async fn get_network_remediation(&self) -> String {
        let mut remediation = Vec::new();

        // Check for common environment variables and provide specific advice
        if std::env::var("GH_TOKEN").is_err() {
            remediation.push("Set GH_TOKEN environment variable for GitHub API access");
        }

        if std::env::var("HTTPS_PROXY").is_ok() || std::env::var("HTTP_PROXY").is_ok() {
            remediation.push("Verify proxy settings allow GitHub access");
        }

        // Check if we're in a container/CI environment
        if std::env::var("CI").is_ok() || std::env::var("CONTAINER").is_ok() {
            remediation.push("Ensure container/CI environment has network access to github.com");
        }

        if remediation.is_empty() {
            "Check network connectivity to github.com and verify GH_TOKEN if using private repositories".to_string()
        } else {
            remediation.join("; ")
        }
    }

    /// Load bundle with fallback to embedded schemas
    pub async fn load_with_fallback(&self, tag: Option<String>) -> Result<ContractBundle> {
        let tag_str = if let Some(ref t) = tag {
            t.clone()
        } else {
            std::env::var("DEMON_CONTRACTS_TAG").unwrap_or_else(|_| "contracts-latest".to_string())
        };

        match self.load_bundle(tag).await {
            Ok(bundle) => Ok(bundle),
            Err(e) => {
                warn!(
                    "Failed to load contract bundle, falling back to embedded schemas: {}",
                    e
                );

                // Emit fallback activated audit event
                let remediation = format!(
                    "{}; or disable bundle loading with DEMON_SKIP_CONTRACT_BUNDLE=1",
                    self.get_network_remediation().await
                );
                let event = BundleAuditor::fallback_activated(
                    tag_str.clone(),
                    e.to_string(),
                    remediation.clone(),
                );
                BundleAuditor::emit_event(event);

                // Add fallback alert
                self.add_alert(
                    AlertSeverity::Warning,
                    format!("Using fallback schemas due to bundle load failure: {}", e),
                    remediation,
                )
                .await;
                self.update_status(BundleStatus::UsingFallback).await;

                {
                    let mut state = self.state.write().await;
                    state.using_fallback = true;
                }

                self.load_embedded_schemas().await
            }
        }
    }

    async fn load_embedded_schemas(&self) -> Result<ContractBundle> {
        // This would load schemas from the compiled binary or local files
        // For now, return an error indicating embedded schemas are not implemented
        anyhow::bail!("Embedded schemas fallback not yet implemented")
    }

    /// Extract schemas to a directory for file-based access
    pub async fn extract_to_dir(&self, bundle: &ContractBundle, target_dir: &Path) -> Result<()> {
        // Create schema directories
        let schema_dir = target_dir.join("schemas");
        let config_dir = target_dir.join("config");
        let wit_dir = target_dir.join("wit");

        fs::create_dir_all(&schema_dir)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&wit_dir)?;

        // Write schemas
        for (name, content) in &bundle.schemas {
            let path = if name.contains("-config.") {
                config_dir.join(name)
            } else {
                schema_dir.join(name)
            };
            fs::write(&path, content)
                .with_context(|| format!("Failed to write schema: {}", name))?;
        }

        // Write WIT definitions
        for (name, content) in &bundle.wit_definitions {
            let path = wit_dir.join(name);
            fs::write(&path, content).with_context(|| format!("Failed to write WIT: {}", name))?;
        }

        debug!(
            "Extracted {} schemas and {} WIT definitions to {}",
            bundle.schemas.len(),
            bundle.wit_definitions.len(),
            target_dir.display()
        );

        Ok(())
    }
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bundle_loader_initialization() {
        let loader = BundleLoader::new(None);
        assert!(loader.metadata().await.is_none());
    }

    #[tokio::test]
    async fn test_cache_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("contracts");
        let loader = BundleLoader::new(Some(cache_dir.clone()));

        // This will fail since we don't have a real bundle to download
        // but it should create the directory structure
        let _ = loader.load_bundle(Some("test-tag".to_string())).await;

        assert!(cache_dir.exists());
    }
}
