//! Storage utilities for graph capsule
//!
//! This module provides helpers to interact with graph storage (GRAPH_COMMITS stream
//! and GRAPH_TAGS KV bucket), including graph materialization from commit history.

use crate::types::{EdgeSnapshot, GraphScope, Mutation, NodeSnapshot, TaggedCommit};
use anyhow::{Context, Result};
use async_nats::jetstream::{self, consumer::DeliverPolicy, kv::Store};
use chrono::Utc;
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

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

/// Commit event payload from GRAPH_COMMITS stream (internal representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitEvent {
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

/// Materialized graph state reconstructed from commits
#[derive(Debug, Clone)]
pub struct GraphStore {
    pub nodes: HashMap<String, NodeSnapshot>,
    pub edges: HashMap<String, EdgeSnapshot>,
    pub commit_count: usize,
}

impl GraphStore {
    /// Create a new empty graph store
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            commit_count: 0,
        }
    }

    /// Apply a single mutation to the graph state
    fn apply_mutation(&mut self, mutation: &Mutation) {
        match mutation {
            Mutation::AddNode {
                node_id,
                labels,
                properties,
            } => {
                self.nodes.insert(
                    node_id.clone(),
                    NodeSnapshot {
                        node_id: node_id.clone(),
                        labels: labels.clone(),
                        properties: properties.clone(),
                    },
                );
            }
            Mutation::UpdateNode {
                node_id,
                labels,
                properties,
            } => {
                self.nodes.insert(
                    node_id.clone(),
                    NodeSnapshot {
                        node_id: node_id.clone(),
                        labels: labels.clone(),
                        properties: properties.clone(),
                    },
                );
            }
            Mutation::RemoveNode { node_id } => {
                self.nodes.remove(node_id);
                // Also remove edges connected to this node
                self.edges
                    .retain(|_, edge| edge.from_node != *node_id && edge.to_node != *node_id);
            }
            Mutation::AddEdge {
                edge_id,
                from,
                to,
                label,
                properties,
            } => {
                self.edges.insert(
                    edge_id.clone(),
                    EdgeSnapshot {
                        edge_id: edge_id.clone(),
                        from_node: from.clone(),
                        to_node: to.clone(),
                        label: label.clone(),
                        properties: properties.clone(),
                    },
                );
            }
            Mutation::UpdateEdge {
                edge_id,
                from,
                to,
                label,
                properties,
            } => {
                self.edges.insert(
                    edge_id.clone(),
                    EdgeSnapshot {
                        edge_id: edge_id.clone(),
                        from_node: from.clone(),
                        to_node: to.clone(),
                        label: label.clone(),
                        properties: properties.clone(),
                    },
                );
            }
            Mutation::RemoveEdge { edge_id } => {
                self.edges.remove(edge_id);
            }
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &str) -> Option<NodeSnapshot> {
        self.nodes.get(node_id).cloned()
    }

    /// Find neighbors of a node up to a given depth using BFS
    pub fn neighbors(&self, start_node_id: &str, max_depth: u32) -> Vec<NodeSnapshot> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Check if start node exists
        if !self.nodes.contains_key(start_node_id) {
            return result;
        }

        // BFS traversal
        queue.push_back((start_node_id.to_string(), 0));
        visited.insert(start_node_id.to_string());

        while let Some((node_id, depth)) = queue.pop_front() {
            if depth > 0 {
                // Don't include the start node itself
                if let Some(node) = self.nodes.get(&node_id) {
                    result.push(node.clone());
                }
            }

            // Explore neighbors if we haven't exceeded depth
            if depth < max_depth {
                for edge in self.edges.values() {
                    let neighbor_id = if edge.from_node == node_id {
                        Some(&edge.to_node)
                    } else if edge.to_node == node_id {
                        Some(&edge.from_node)
                    } else {
                        None
                    };

                    if let Some(neighbor_id) = neighbor_id {
                        if !visited.contains(neighbor_id) {
                            visited.insert(neighbor_id.clone());
                            queue.push_back((neighbor_id.clone(), depth + 1));
                        }
                    }
                }
            }
        }

        result
    }

    /// Check if a path exists between two nodes within max_depth
    pub fn path_exists(&self, from: &str, to: &str, max_depth: u32) -> bool {
        if !self.nodes.contains_key(from) || !self.nodes.contains_key(to) {
            return false;
        }

        if from == to {
            return true;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((from.to_string(), 0));
        visited.insert(from.to_string());

        while let Some((node_id, depth)) = queue.pop_front() {
            if node_id == to {
                return true;
            }

            if depth < max_depth {
                for edge in self.edges.values() {
                    let neighbor_id = if edge.from_node == node_id {
                        Some(&edge.to_node)
                    } else if edge.to_node == node_id {
                        Some(&edge.from_node)
                    } else {
                        None
                    };

                    if let Some(neighbor_id) = neighbor_id {
                        if !visited.contains(neighbor_id) {
                            visited.insert(neighbor_id.clone());
                            queue.push_back((neighbor_id.clone(), depth + 1));
                        }
                    }
                }
            }
        }

        false
    }
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum number of commits to replay (safety limit)
const MAX_COMMITS_TO_REPLAY: usize = 10_000;

/// Materialize graph state up to a given commit by replaying all commits in order
///
/// This function fetches commits from the GRAPH_COMMITS stream and replays them
/// in chronological order to reconstruct the graph state at the specified commit.
pub async fn materialize_graph_at_commit(
    scope: &GraphScope,
    target_commit_id: &str,
) -> Result<GraphStore> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client);

    let stream = js
        .get_stream("GRAPH_COMMITS")
        .await
        .context("Failed to get GRAPH_COMMITS stream")?;

    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    tracing::debug!(
        "Materializing graph for scope {}/{}/{}/{} at commit {}",
        scope.tenant_id,
        scope.project_id,
        scope.namespace,
        scope.graph_id,
        target_commit_id
    );

    // Create consumer to fetch commits
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await
        .context("Failed to create consumer for graph materialization")?;

    // Fetch all commits up to limit
    let mut batch = consumer
        .batch()
        .max_messages(MAX_COMMITS_TO_REPLAY)
        .expires(Duration::from_secs(10))
        .messages()
        .await?;

    let mut commits = Vec::new();

    while let Some(result) = batch.next().await {
        let msg = result.map_err(|e| anyhow::anyhow!("Failed to fetch message: {}", e))?;

        // Parse event
        match serde_json::from_slice::<CommitEvent>(&msg.payload) {
            Ok(event) => {
                tracing::debug!("Found event: {} for graph: {}", event.event, event.graph_id);
                if event.event == "graph.commit.created:v1" {
                    // Only include commit events for this graph
                    if event.graph_id == scope.graph_id {
                        tracing::debug!(
                            "Including commit {} for graph {}",
                            event.commit_id,
                            event.graph_id
                        );
                        commits.push(event);
                    } else {
                        tracing::debug!(
                            "Skipping commit for different graph: {} != {}",
                            event.graph_id,
                            scope.graph_id
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to deserialize commit event: {}", e);
            }
        }
    }

    tracing::info!(
        "Fetched {} commits from stream for graph {}",
        commits.len(),
        scope.graph_id
    );

    if commits.len() >= MAX_COMMITS_TO_REPLAY {
        anyhow::bail!(
            "Commit count ({}) exceeds safety limit ({}). Graph too large for replay.",
            commits.len(),
            MAX_COMMITS_TO_REPLAY
        );
    }

    // Sort commits by timestamp (chronological order)
    commits.sort_by(|a, b| a.ts.cmp(&b.ts));

    // Build graph state by replaying commits up to target
    let mut store = GraphStore::new();
    let mut found_target = false;

    for event in commits {
        // Parse mutations and apply them
        for mutation_json in &event.mutations {
            match serde_json::from_value::<Mutation>(mutation_json.clone()) {
                Ok(mutation) => {
                    store.apply_mutation(&mutation);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to deserialize mutation in commit {}: {}. JSON: {}",
                        event.commit_id,
                        e,
                        mutation_json
                    );
                }
            }
        }

        store.commit_count += 1;

        // Stop if we've reached the target commit
        if event.commit_id == target_commit_id {
            found_target = true;
            tracing::info!(
                "Reached target commit {}. Graph state: {} nodes, {} edges",
                target_commit_id,
                store.nodes.len(),
                store.edges.len()
            );
            break;
        }
    }

    if !found_target {
        anyhow::bail!("Commit {} not found in stream", target_commit_id);
    }

    tracing::info!(
        "Materialized graph at commit {} with {} nodes, {} edges from {} commits",
        target_commit_id,
        store.nodes.len(),
        store.edges.len(),
        store.commit_count
    );

    Ok(store)
}
