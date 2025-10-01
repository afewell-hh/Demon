//! Query layer for graph commits and tags
//!
//! Provides read-only query operations against graph commit stream and tag KV bucket.

use anyhow::{Context, Result};
use async_nats::jetstream;
use capsules_graph::{CommitResult, GraphScope, TaggedCommit};
use chrono::Utc;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Commit event payload from GRAPH_COMMITS stream
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitEvent {
    pub event: String,
    pub graph_id: String,
    pub tenant_id: String,
    pub project_id: String,
    pub namespace: String,
    pub commit_id: String,
    pub parent_commit_id: Option<String>,
    pub ts: String,
    pub mutations: Vec<serde_json::Value>,
}

impl CommitEvent {
    /// Convert to CommitResult for API responses
    pub fn to_commit_result(&self) -> Result<CommitResult> {
        let timestamp = chrono::DateTime::parse_from_rfc3339(&self.ts)
            .context("Failed to parse timestamp")?
            .with_timezone(&Utc);

        Ok(CommitResult {
            commit_id: self.commit_id.clone(),
            parent_commit_id: self.parent_commit_id.clone(),
            mutations_count: self.mutations.len(),
            timestamp,
        })
    }
}

/// Get NATS URL from environment
fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

/// Build subject for graph commits within a scope
pub fn graph_subject(scope: &GraphScope, commit_id: Option<&str>) -> String {
    let base = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    if let Some(cid) = commit_id {
        format!("{}:{}", base, cid)
    } else {
        base
    }
}

/// Fetch a single commit by ID from GRAPH_COMMITS stream
///
/// Uses a filtered consumer to locate the commit event by subject + commit ID.
pub async fn get_commit_by_id(scope: &GraphScope, commit_id: &str) -> Result<Option<CommitEvent>> {
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let stream = js
        .get_stream("GRAPH_COMMITS")
        .await
        .context("Failed to get GRAPH_COMMITS stream")?;

    let subject = graph_subject(scope, None);

    // Create a filtered consumer for this scope
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await
        .context("Failed to create consumer for commit query")?;

    // Fetch messages and search for matching commit ID
    let mut batch = consumer
        .batch()
        .max_messages(1000) // reasonable limit for search
        .expires(Duration::from_secs(5))
        .messages()
        .await?;

    while let Some(result) = batch.next().await {
        let msg = result.map_err(|e| anyhow::anyhow!("Failed to fetch message: {}", e))?;
        let event: CommitEvent =
            serde_json::from_slice(&msg.payload).context("Failed to deserialize commit event")?;

        if event.commit_id == commit_id {
            return Ok(Some(event));
        }
    }

    Ok(None)
}

/// List recent commits for a graph scope
///
/// Returns up to `limit` commits (default: 50, max: 1000).
pub async fn list_commits(scope: &GraphScope, limit: Option<usize>) -> Result<Vec<CommitEvent>> {
    let limit = limit.unwrap_or(50).min(1000);

    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let stream = js
        .get_stream("GRAPH_COMMITS")
        .await
        .context("Failed to get GRAPH_COMMITS stream")?;

    let subject = graph_subject(scope, None);

    // Create consumer for this scope
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await
        .context("Failed to create consumer for commit listing")?;

    // Fetch messages
    let mut batch = consumer
        .batch()
        .max_messages(limit)
        .expires(Duration::from_secs(5))
        .messages()
        .await?;

    let mut commits = Vec::new();

    while let Some(result) = batch.next().await {
        match result {
            Ok(msg) => match serde_json::from_slice::<CommitEvent>(&msg.payload) {
                Ok(event) => {
                    // Only include commit events (not tag events)
                    if event.event == "graph.commit.created:v1" {
                        commits.push(event);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize event: {}", e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to fetch message: {}", e);
            }
        }
    }

    // Sort by timestamp descending (most recent first)
    commits.sort_by(|a, b| b.ts.cmp(&a.ts));

    Ok(commits)
}

/// Retrieve tag state from GRAPH_TAGS KV bucket
///
/// Returns the commit ID associated with the tag, or None if tag doesn't exist.
pub async fn get_tag(scope: &GraphScope, tag: &str) -> Result<Option<TaggedCommit>> {
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Get KV bucket
    let kv = capsules_graph::storage::ensure_graph_tags_kv(&js)
        .await
        .context("Failed to get GRAPH_TAGS KV bucket")?;

    // Build key
    let key = format!(
        "{}/{}/{}/{}/{}",
        scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id, tag
    );

    // Try to get the tag
    match kv.get(&key).await {
        Ok(Some(entry)) => {
            let bytes: Vec<u8> = entry.into();
            let commit_id =
                String::from_utf8(bytes).context("Invalid UTF-8 in stored tag value")?;

            Ok(Some(TaggedCommit {
                tag: tag.to_string(),
                commit_id,
                timestamp: Utc::now().to_rfc3339(), // Use current time as placeholder
            }))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to retrieve tag: {}", e)),
    }
}

/// List all tags for a given scope
///
/// Reuses capsule storage helper for consistency.
pub async fn list_tags(scope: &GraphScope) -> Result<Vec<TaggedCommit>> {
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let kv = capsules_graph::storage::ensure_graph_tags_kv(&js)
        .await
        .context("Failed to get GRAPH_TAGS KV bucket")?;

    capsules_graph::storage::list_tags(&kv, scope)
        .await
        .context("Failed to list tags from KV")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_subject_without_commit_id() {
        let scope = GraphScope {
            tenant_id: "tenant-1".to_string(),
            project_id: "proj-1".to_string(),
            namespace: "ns-1".to_string(),
            graph_id: "graph-1".to_string(),
        };

        let subject = graph_subject(&scope, None);
        assert_eq!(subject, "demon.graph.v1.tenant-1.proj-1.ns-1.commit");
    }

    #[test]
    fn graph_subject_with_commit_id() {
        let scope = GraphScope {
            tenant_id: "tenant-1".to_string(),
            project_id: "proj-1".to_string(),
            namespace: "ns-1".to_string(),
            graph_id: "graph-1".to_string(),
        };

        let subject = graph_subject(&scope, Some("abc123"));
        assert_eq!(subject, "demon.graph.v1.tenant-1.proj-1.ns-1.commit:abc123");
    }
}
