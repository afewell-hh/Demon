//! Graph capsule implementing demon-graph.wit interface
//!
//! This capsule provides graph commit and tag operations with event emission
//! to NATS JetStream. Query operations (get-node, neighbors, path-exists) are
//! implemented via graph materialization from the commit stream.

use chrono::Utc;
use envelope::{AsEnvelope, Diagnostic, DiagnosticLevel, ResultEnvelope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub mod events;
pub mod storage;
pub mod types;

pub use types::*;

/// Result of a commit operation
#[derive(Serialize, Deserialize, AsEnvelope, Debug, Clone)]
pub struct CommitResult {
    pub commit_id: String,
    pub parent_commit_id: Option<String>,
    pub mutations_count: usize,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Result of a tag operation
#[derive(Serialize, Deserialize, AsEnvelope, Debug, Clone)]
pub struct TagResult {
    pub tag: String,
    pub commit_id: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub action: TagAction,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagAction {
    Set,
    Delete,
}

/// Generate deterministic SHA256 commit ID from scope, parent, and mutations
///
/// Commit ID format: sha256(tenant||project||namespace||parent||sorted_mutations_json)
pub fn compute_commit_id(
    scope: &GraphScope,
    parent_commit_id: Option<&str>,
    mutations: &[Mutation],
) -> String {
    let mut hasher = Sha256::new();

    // Hash scope components
    hasher.update(scope.tenant_id.as_bytes());
    hasher.update(b"|");
    hasher.update(scope.project_id.as_bytes());
    hasher.update(b"|");
    hasher.update(scope.namespace.as_bytes());
    hasher.update(b"|");
    hasher.update(scope.graph_id.as_bytes());
    hasher.update(b"|");

    // Hash parent if present
    if let Some(parent) = parent_commit_id {
        hasher.update(parent.as_bytes());
    }
    hasher.update(b"|");

    // Sort mutations for determinism (by JSON repr)
    let mut sorted_mutations: Vec<_> = mutations
        .iter()
        .map(|m| serde_json::to_string(m).unwrap_or_default())
        .collect();
    sorted_mutations.sort();

    for m in sorted_mutations {
        hasher.update(m.as_bytes());
        hasher.update(b"|");
    }

    let result = hasher.finalize();
    hex::encode(result)
}

/// Create a new graph by seeding an initial commit
///
/// This emits a graph.commit.created:v1 event and returns the commit ID.
pub async fn create(scope: GraphScope, seed: Vec<Mutation>) -> ResultEnvelope<CommitResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    if seed.is_empty() {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                "Cannot create graph with empty seed mutations".to_string(),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code("Seed mutations cannot be empty", "EMPTY_SEED")
            .build()
            .expect("Valid envelope");
    }

    // Compute commit ID (no parent for initial commit)
    let commit_id = compute_commit_id(&scope, None, &seed);

    // Emit graph.commit.created:v1 event
    if let Err(e) = events::emit_commit_created(&scope, &commit_id, None, &seed, timestamp).await {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                format!("Failed to emit commit event: {}", e),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code(
                format!("Event emission error: {}", e),
                "EVENT_EMISSION_FAILED",
            )
            .build()
            .expect("Valid envelope");
    }

    let result = CommitResult {
        commit_id,
        parent_commit_id: None,
        mutations_count: seed.len(),
        timestamp,
    };

    let mut builder = ResultEnvelope::builder()
        .success(result)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!("Graph created with {} seed mutations", seed.len()),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("mutations".to_string(), seed.len() as i64);

    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// Commit a new set of mutations referencing an optional parent commit
///
/// This emits a graph.commit.created:v1 event and returns the commit ID.
pub async fn commit(
    scope: GraphScope,
    parent_ref: Option<String>,
    mutations: Vec<Mutation>,
) -> ResultEnvelope<CommitResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    if mutations.is_empty() {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                "Cannot commit with empty mutations".to_string(),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code("Mutations cannot be empty", "EMPTY_MUTATIONS")
            .build()
            .expect("Valid envelope");
    }

    // Compute commit ID
    let commit_id = compute_commit_id(&scope, parent_ref.as_deref(), &mutations);

    // Emit graph.commit.created:v1 event
    if let Err(e) = events::emit_commit_created(
        &scope,
        &commit_id,
        parent_ref.as_deref(),
        &mutations,
        timestamp,
    )
    .await
    {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                format!("Failed to emit commit event: {}", e),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code(
                format!("Event emission error: {}", e),
                "EVENT_EMISSION_FAILED",
            )
            .build()
            .expect("Valid envelope");
    }

    let result = CommitResult {
        commit_id,
        parent_commit_id: parent_ref,
        mutations_count: mutations.len(),
        timestamp,
    };

    let mut builder = ResultEnvelope::builder()
        .success(result)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!("Committed {} mutations", mutations.len()),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("mutations".to_string(), mutations.len() as i64);

    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// Attach or update a tag to point at a commit
///
/// This emits a graph.tag.updated:v1 event and stores the tag in KV.
pub async fn tag(scope: GraphScope, tag: String, commit_id: String) -> ResultEnvelope<TagResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    // Connect to NATS and get KV store
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = match async_nats::connect(&url).await {
        Ok(c) => c,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to connect to NATS: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("NATS connection error: {}", e),
                    "NATS_CONNECTION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };
    let js = async_nats::jetstream::new(client);

    // Ensure KV bucket exists
    let kv = match storage::ensure_graph_tags_kv(&js).await {
        Ok(kv) => kv,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to create KV bucket: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("KV bucket error: {}", e), "KV_BUCKET_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    // Store tag in KV (get previous commit if updating)
    let _previous_commit = match storage::put_tag(&kv, &scope, &tag, &commit_id).await {
        Ok(prev) => prev,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to store tag in KV: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("Tag storage error: {}", e), "TAG_STORAGE_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    // Emit graph.tag.updated:v1 event (action=set)
    if let Err(e) =
        events::emit_tag_updated(&scope, &tag, Some(&commit_id), TagAction::Set, timestamp).await
    {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                format!("Failed to emit tag event: {}", e),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code(format!("Tag event error: {}", e), "EVENT_EMISSION_FAILED")
            .build()
            .expect("Valid envelope");
    }

    let result = TagResult {
        tag,
        commit_id,
        timestamp,
        action: TagAction::Set,
    };

    let mut builder = ResultEnvelope::builder()
        .success(result)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            "Tag updated successfully".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters: HashMap::new(),
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// Delete a tag from the graph scope
///
/// This emits a graph.tag.updated:v1 event with action=delete and removes the tag from KV.
pub async fn delete_tag(scope: GraphScope, tag: String) -> ResultEnvelope<TagResult> {
    let start = std::time::Instant::now();
    let timestamp = Utc::now();

    // Connect to NATS and get KV store
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = match async_nats::connect(&url).await {
        Ok(c) => c,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to connect to NATS: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("NATS connection error: {}", e),
                    "NATS_CONNECTION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };
    let js = async_nats::jetstream::new(client);

    // Ensure KV bucket exists
    let kv = match storage::ensure_graph_tags_kv(&js).await {
        Ok(kv) => kv,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to get KV bucket: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("KV bucket error: {}", e), "KV_BUCKET_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    // Delete tag from KV
    let deleted_commit = match storage::delete_tag(&kv, &scope, &tag).await {
        Ok(commit) => commit,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to delete tag from KV: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("Tag deletion error: {}", e), "TAG_DELETION_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    // If tag didn't exist, return error
    let commit_id = match deleted_commit {
        Some(id) => id,
        None => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Tag '{}' not found", tag),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("Tag '{}' does not exist", tag), "TAG_NOT_FOUND")
                .build()
                .expect("Valid envelope");
        }
    };

    // Emit graph.tag.updated:v1 event (action=delete)
    if let Err(e) = events::emit_tag_updated(&scope, &tag, None, TagAction::Delete, timestamp).await
    {
        let builder = ResultEnvelope::builder()
            .add_diagnostic(Diagnostic::new(
                DiagnosticLevel::Error,
                format!("Failed to emit tag event: {}", e),
            ))
            .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

        return builder
            .error_with_code(format!("Tag event error: {}", e), "EVENT_EMISSION_FAILED")
            .build()
            .expect("Valid envelope");
    }

    let result = TagResult {
        tag,
        commit_id,
        timestamp,
        action: TagAction::Delete,
    };

    let mut builder = ResultEnvelope::builder()
        .success(result)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            "Tag deleted successfully".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters: HashMap::new(),
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// List all tags associated with the graph scope
///
/// Scans the GRAPH_TAGS KV bucket and returns all tags for the given scope.
pub async fn list_tags(scope: GraphScope) -> ResultEnvelope<Vec<TaggedCommit>> {
    let start = std::time::Instant::now();

    // Connect to NATS and get KV store
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = match async_nats::connect(&url).await {
        Ok(c) => c,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to connect to NATS: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("NATS connection error: {}", e),
                    "NATS_CONNECTION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };
    let js = async_nats::jetstream::new(client);

    // Ensure KV bucket exists
    let kv = match storage::ensure_graph_tags_kv(&js).await {
        Ok(kv) => kv,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to get KV bucket: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("KV bucket error: {}", e), "KV_BUCKET_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    // List tags from KV
    let tags = match storage::list_tags(&kv, &scope).await {
        Ok(tags) => tags,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to list tags from KV: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(format!("Tag listing error: {}", e), "TAG_LISTING_FAILED")
                .build()
                .expect("Valid envelope");
        }
    };

    let mut builder = ResultEnvelope::builder()
        .success(tags)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            "Tags listed successfully".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters: HashMap::new(),
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

// Query operations using graph materialization

/// Retrieve a node snapshot for a given commit and node identifier
///
/// Materializes the graph state at the specified commit and returns the node if it exists.
pub async fn get_node(
    scope: GraphScope,
    commit_id: String,
    node_id: String,
) -> ResultEnvelope<Option<NodeSnapshot>> {
    let start = std::time::Instant::now();

    // Materialize graph at commit
    let graph = match storage::materialize_graph_at_commit(&scope, &commit_id).await {
        Ok(g) => g,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to materialize graph: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("Graph materialization error: {}", e),
                    "MATERIALIZATION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };

    let node = graph.get_node(&node_id);

    let mut builder = ResultEnvelope::builder()
        .success(node)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!("Query completed for node '{}'", node_id),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("nodes_in_graph".to_string(), graph.nodes.len() as i64);
    counters.insert("edges_in_graph".to_string(), graph.edges.len() as i64);
    counters.insert("commits_replayed".to_string(), graph.commit_count as i64);

    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// List neighboring nodes up to the specified depth from the starting node
///
/// Uses BFS to traverse the graph and find all nodes reachable within the specified depth.
pub async fn neighbors(
    scope: GraphScope,
    commit_id: String,
    node_id: String,
    depth: u32,
) -> ResultEnvelope<Vec<NodeSnapshot>> {
    let start = std::time::Instant::now();

    // Materialize graph at commit
    let graph = match storage::materialize_graph_at_commit(&scope, &commit_id).await {
        Ok(g) => g,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to materialize graph: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("Graph materialization error: {}", e),
                    "MATERIALIZATION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };

    let neighbors = graph.neighbors(&node_id, depth);

    let mut builder = ResultEnvelope::builder()
        .success(neighbors.clone())
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!(
                "Found {} neighbors within depth {} from node '{}'",
                neighbors.len(),
                depth,
                node_id
            ),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("nodes_in_graph".to_string(), graph.nodes.len() as i64);
    counters.insert("edges_in_graph".to_string(), graph.edges.len() as i64);
    counters.insert("commits_replayed".to_string(), graph.commit_count as i64);
    counters.insert("neighbors_found".to_string(), neighbors.len() as i64);
    counters.insert("max_depth".to_string(), depth as i64);

    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

/// Determine whether a path exists between two nodes within the depth constraint
///
/// Uses BFS to search for a path between the two nodes within the specified maximum depth.
pub async fn path_exists(
    scope: GraphScope,
    commit_id: String,
    from: String,
    to: String,
    max_depth: u32,
) -> ResultEnvelope<bool> {
    let start = std::time::Instant::now();

    // Materialize graph at commit
    let graph = match storage::materialize_graph_at_commit(&scope, &commit_id).await {
        Ok(g) => g,
        Err(e) => {
            let builder = ResultEnvelope::builder()
                .add_diagnostic(Diagnostic::new(
                    DiagnosticLevel::Error,
                    format!("Failed to materialize graph: {}", e),
                ))
                .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

            return builder
                .error_with_code(
                    format!("Graph materialization error: {}", e),
                    "MATERIALIZATION_FAILED",
                )
                .build()
                .expect("Valid envelope");
        }
    };

    let exists = graph.path_exists(&from, &to, max_depth);

    let mut builder = ResultEnvelope::builder()
        .success(exists)
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Info,
            format!(
                "Path {} between '{}' and '{}' within depth {}",
                if exists { "exists" } else { "not found" },
                from,
                to,
                max_depth
            ),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    let duration = start.elapsed();
    let mut counters = HashMap::new();
    counters.insert("nodes_in_graph".to_string(), graph.nodes.len() as i64);
    counters.insert("edges_in_graph".to_string(), graph.edges.len() as i64);
    counters.insert("commits_replayed".to_string(), graph.commit_count as i64);
    counters.insert("max_depth".to_string(), max_depth as i64);

    builder = builder.metrics(envelope::Metrics {
        duration: Some(envelope::DurationMetrics {
            total_ms: Some(duration.as_secs_f64() * 1000.0),
            phases: HashMap::new(),
        }),
        resources: None,
        counters,
        custom: None,
    });

    builder.build().expect("Valid envelope")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_id_is_deterministic() {
        let scope = GraphScope {
            tenant_id: "tenant-1".to_string(),
            project_id: "proj-1".to_string(),
            namespace: "ns-1".to_string(),
            graph_id: "graph-1".to_string(),
        };

        let mutations = vec![
            Mutation::AddNode {
                node_id: "node-1".to_string(),
                labels: vec!["Label1".to_string()],
                properties: vec![],
            },
            Mutation::AddNode {
                node_id: "node-2".to_string(),
                labels: vec!["Label2".to_string()],
                properties: vec![],
            },
        ];

        let commit_id_1 = compute_commit_id(&scope, None, &mutations);
        let commit_id_2 = compute_commit_id(&scope, None, &mutations);

        assert_eq!(commit_id_1, commit_id_2);
        assert_eq!(commit_id_1.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn commit_id_differs_with_parent() {
        let scope = GraphScope {
            tenant_id: "tenant-1".to_string(),
            project_id: "proj-1".to_string(),
            namespace: "ns-1".to_string(),
            graph_id: "graph-1".to_string(),
        };

        let mutations = vec![Mutation::AddNode {
            node_id: "node-1".to_string(),
            labels: vec![],
            properties: vec![],
        }];

        let id_without_parent = compute_commit_id(&scope, None, &mutations);
        let id_with_parent = compute_commit_id(&scope, Some("parent-abc"), &mutations);

        assert_ne!(id_without_parent, id_with_parent);
    }

    #[test]
    fn commit_id_is_mutation_order_independent() {
        let scope = GraphScope {
            tenant_id: "tenant-1".to_string(),
            project_id: "proj-1".to_string(),
            namespace: "ns-1".to_string(),
            graph_id: "graph-1".to_string(),
        };

        let mutations1 = vec![
            Mutation::AddNode {
                node_id: "node-a".to_string(),
                labels: vec![],
                properties: vec![],
            },
            Mutation::AddNode {
                node_id: "node-b".to_string(),
                labels: vec![],
                properties: vec![],
            },
        ];

        let mutations2 = vec![
            Mutation::AddNode {
                node_id: "node-b".to_string(),
                labels: vec![],
                properties: vec![],
            },
            Mutation::AddNode {
                node_id: "node-a".to_string(),
                labels: vec![],
                properties: vec![],
            },
        ];

        let id1 = compute_commit_id(&scope, None, &mutations1);
        let id2 = compute_commit_id(&scope, None, &mutations2);

        assert_eq!(id1, id2, "Commit ID should be order-independent");
    }
}
