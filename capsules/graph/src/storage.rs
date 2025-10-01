//! Storage utilities for graph capsule
//!
//! This module provides helpers to interact with graph storage (GRAPH_COMMITS stream
//! and GRAPH_TAGS KV bucket).

use crate::types::{GraphScope, TaggedCommit};
use anyhow::{Context, Result};
use async_nats::jetstream::{self, kv::Store};
use chrono::Utc;
use futures_util::TryStreamExt;

/// Ensure GRAPH_TAGS KV bucket exists
///
/// Creates or gets the KV bucket used for storing tag-to-commit mappings.
pub async fn ensure_graph_tags_kv(js: &jetstream::Context) -> Result<Store> {
    // Try to get existing KV bucket first
    match js.get_key_value("GRAPH_TAGS").await {
        Ok(kv) => Ok(kv),
        Err(_) => {
            // If it doesn't exist, create it
            let kv = js
                .create_key_value(jetstream::kv::Config {
                    bucket: "GRAPH_TAGS".to_string(),
                    description: "Graph tag to commit ID mappings".to_string(),
                    history: 10,
                    storage: jetstream::stream::StorageType::File,
                    ..Default::default()
                })
                .await
                .context("Failed to create GRAPH_TAGS KV bucket")?;
            Ok(kv)
        }
    }
}

/// Build KV key for a tag within a scope
///
/// Key format: {tenant}/{project}/{namespace}/{graph}/{tag}
fn tag_key(scope: &GraphScope, tag: &str) -> String {
    format!(
        "{}/{}/{}/{}/{}",
        scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id, tag
    )
}

/// Store a tag-to-commit mapping in KV
///
/// Returns the previous commit ID if the tag already existed, None otherwise.
pub async fn put_tag(
    kv: &Store,
    scope: &GraphScope,
    tag: &str,
    commit_id: &str,
) -> Result<Option<String>> {
    let key = tag_key(scope, tag);

    // Check if tag already exists
    let previous_commit = match kv.get(&key).await {
        Ok(Some(entry)) => {
            let bytes = entry.into();
            Some(String::from_utf8(bytes).context("Invalid UTF-8 in stored tag value")?)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("Error checking existing tag {}: {}", key, e);
            None
        }
    };

    // Store new value (convert to owned bytes)
    kv.put(&key, commit_id.as_bytes().to_vec().into())
        .await
        .context("Failed to store tag in KV")?;

    Ok(previous_commit)
}

/// Delete a tag from KV storage
///
/// Returns the commit ID that was associated with the tag if it existed.
pub async fn delete_tag(kv: &Store, scope: &GraphScope, tag: &str) -> Result<Option<String>> {
    let key = tag_key(scope, tag);

    // Get current value before deletion
    let commit_id = match kv.get(&key).await {
        Ok(Some(entry)) => {
            let bytes = entry.into();
            Some(String::from_utf8(bytes).context("Invalid UTF-8 in stored tag value")?)
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("Error checking existing tag {} for deletion: {}", key, e);
            None
        }
    };

    // Delete if exists
    if commit_id.is_some() {
        kv.delete(&key)
            .await
            .context("Failed to delete tag from KV")?;
    }

    Ok(commit_id)
}

/// List all tags for a given scope
///
/// Scans KV bucket for keys matching the scope prefix and returns TaggedCommit entries.
pub async fn list_tags(kv: &Store, scope: &GraphScope) -> Result<Vec<TaggedCommit>> {
    let prefix = format!(
        "{}/{}/{}/{}/",
        scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let mut keys = kv.keys().await.context("Failed to list KV keys")?;

    let mut tags = Vec::new();

    while let Some(result) = keys.try_next().await? {
        let key = result;
        if key.starts_with(&prefix) {
            // Extract tag name from key
            if let Some(tag_name) = key.strip_prefix(&prefix) {
                // Get the value (commit ID)
                if let Ok(Some(entry)) = kv.get(&key).await {
                    let bytes: Vec<u8> = entry.into();
                    if let Ok(commit_id) = String::from_utf8(bytes) {
                        // Use the KV entry creation time as timestamp
                        // Note: In a real implementation, we might store metadata alongside
                        // For now, use current time as a placeholder
                        tags.push(TaggedCommit {
                            tag: tag_name.to_string(),
                            commit_id,
                            timestamp: Utc::now().to_rfc3339(),
                        });
                    }
                }
            }
        }
    }

    // Sort by tag name for consistent ordering
    tags.sort_by(|a, b| a.tag.cmp(&b.tag));

    Ok(tags)
}
