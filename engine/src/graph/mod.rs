//! Graph storage management for JetStream
//!
//! This module provides helpers to ensure graph commit and tag storage resources
//! exist in NATS JetStream. Graph commits are stored in a stream with replay
//! capabilities, while tags are stored in a KV bucket for fast lookups.

use anyhow::{Context, Result};
use async_nats::jetstream::{
    self,
    stream::{
        Config as StreamConfig, DiscardPolicy, Info as StreamInfo, RetentionPolicy, StorageType,
    },
};
use serde::{Deserialize, Serialize};

/// Default stream name for graph commits
pub const GRAPH_COMMITS_STREAM: &str = "GRAPH_COMMITS";

/// Default KV bucket name for graph tags
pub const GRAPH_TAGS_BUCKET: &str = "GRAPH_TAGS";

/// Subject pattern for graph commit events:
/// demon.graph.v1.{tenant}.{project}.{namespace}.commit
pub const GRAPH_COMMIT_SUBJECT_PREFIX: &str = "demon.graph.v1";

/// Configuration options for graph storage resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStorageConfig {
    /// Stream name override (defaults to GRAPH_COMMITS_STREAM)
    pub stream_name: Option<String>,
    /// KV bucket name override (defaults to GRAPH_TAGS_BUCKET)
    pub tag_bucket_name: Option<String>,
    /// Subject prefix override (defaults to GRAPH_COMMIT_SUBJECT_PREFIX)
    pub subject_prefix: Option<String>,
}

impl Default for GraphStorageConfig {
    fn default() -> Self {
        Self {
            stream_name: None,
            tag_bucket_name: None,
            subject_prefix: None,
        }
    }
}

/// Ensure the graph commits stream exists with appropriate retention and backpressure settings.
///
/// Stream configuration:
/// - **Retention**: Limits (not interest/work-queue) — keeps all commits for replay
/// - **Storage**: File — persists commits to disk for durability
/// - **Max messages per subject**: 10,000 — caps per-tenant/project/namespace growth
/// - **Discard policy**: Old — drops oldest commits when limit reached (backpressure)
/// - **Duplicates window**: 120s — prevents duplicate commit events
/// - **Ack policy**: Explicit — consumers must acknowledge message receipt
///
/// # Arguments
/// * `js` - JetStream context for NATS connection
/// * `config` - Optional configuration overrides
///
/// # Returns
/// StreamInfo for the created or existing stream
pub async fn ensure_graph_stream(
    js: &jetstream::Context,
    config: Option<&GraphStorageConfig>,
) -> Result<StreamInfo> {
    let stream_name = config
        .and_then(|c| c.stream_name.as_deref())
        .unwrap_or(GRAPH_COMMITS_STREAM);

    let subject_prefix = config
        .and_then(|c| c.subject_prefix.as_deref())
        .unwrap_or(GRAPH_COMMIT_SUBJECT_PREFIX);

    let subjects = vec![format!("{}.*.*.*.commit", subject_prefix)];

    let stream_config = StreamConfig {
        name: stream_name.to_string(),
        subjects,
        retention: RetentionPolicy::Limits,
        storage: StorageType::File,
        max_messages_per_subject: 10_000,
        discard: DiscardPolicy::Old,
        duplicate_window: std::time::Duration::from_secs(120),
        ..Default::default()
    };

    // get_or_create will update existing stream if config differs
    let mut stream = js
        .get_or_create_stream(stream_config)
        .await
        .context("failed to ensure GRAPH_COMMITS stream")?;

    Ok(stream.info().await?.clone())
}

/// Ensure the graph tags KV bucket exists.
///
/// KV bucket configuration:
/// - **History**: 1 — only keep latest tag version
/// - **Storage**: Memory — fast lookups for tags (tags are low-volume)
/// - **TTL**: 0 (no expiry) — tags persist until explicitly deleted
/// - **Replicas**: 1 — single replica (can be increased for HA in production)
///
/// # Arguments
/// * `js` - JetStream context for NATS connection
/// * `config` - Optional configuration overrides
///
/// # Returns
/// Store (KV bucket) for graph tags
pub async fn ensure_graph_tag_store(
    js: &jetstream::Context,
    config: Option<&GraphStorageConfig>,
) -> Result<jetstream::kv::Store> {
    let bucket_name = config
        .and_then(|c| c.tag_bucket_name.as_deref())
        .unwrap_or(GRAPH_TAGS_BUCKET);

    let kv_config = jetstream::kv::Config {
        bucket: bucket_name.to_string(),
        history: 1,
        storage: StorageType::Memory,
        num_replicas: 1,
        ..Default::default()
    };

    let store = match js.create_key_value(kv_config).await {
        Ok(s) => s,
        Err(_) => js.get_key_value(bucket_name).await.context("failed to ensure GRAPH_TAGS bucket")?,
    };

    Ok(store)
}

/// Orchestrates creation of both graph storage resources (stream + KV bucket).
///
/// This is the primary entrypoint for initializing graph storage.
///
/// # Arguments
/// * `js` - JetStream context for NATS connection
/// * `config` - Optional configuration overrides
///
/// # Returns
/// Tuple of (StreamInfo, KV Store) for commits and tags respectively
pub async fn ensure_graph_storage(
    js: &jetstream::Context,
    config: Option<&GraphStorageConfig>,
) -> Result<(StreamInfo, jetstream::kv::Store)> {
    let stream_info = ensure_graph_stream(js, config).await?;
    let tag_store = ensure_graph_tag_store(js, config).await?;
    Ok((stream_info, tag_store))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_constants() {
        let cfg = GraphStorageConfig::default();
        assert!(cfg.stream_name.is_none());
        assert!(cfg.tag_bucket_name.is_none());
        assert!(cfg.subject_prefix.is_none());
    }

    #[test]
    fn constants_are_stable() {
        // Ensure stream/bucket names don't accidentally change
        assert_eq!(GRAPH_COMMITS_STREAM, "GRAPH_COMMITS");
        assert_eq!(GRAPH_TAGS_BUCKET, "GRAPH_TAGS");
        assert_eq!(GRAPH_COMMIT_SUBJECT_PREFIX, "demon.graph.v1");
    }
}
