//! Graph capsule implementing demon-graph.wit interface
//!
//! This capsule provides graph commit and tag operations with event emission
//! to NATS JetStream. Query operations (get-node, neighbors, path-exists) are
//! placeholders pending full graph materialization design.

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

/// List all tags associated with the graph scope
///
/// TODO: Implement KV scan for tag prefix
pub async fn list_tags(_scope: GraphScope) -> ResultEnvelope<Vec<TaggedCommit>> {
    let builder = ResultEnvelope::builder()
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Warning,
            "list_tags not yet implemented - returns empty list".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    builder.success(vec![]).build().expect("Valid envelope")
}

// Query operation placeholders (pending graph materialization design)

/// Retrieve a node snapshot for a given commit and node identifier
///
/// TODO: Implement graph query layer
pub async fn get_node(
    _scope: GraphScope,
    _commit_id: String,
    _node_id: String,
) -> ResultEnvelope<Option<NodeSnapshot>> {
    let builder = ResultEnvelope::builder()
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Error,
            "get_node not yet implemented - graph query layer pending".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    builder
        .error_with_code(
            "Graph query operations not yet implemented",
            "NOT_IMPLEMENTED",
        )
        .build()
        .expect("Valid envelope")
}

/// List neighboring nodes up to the specified depth from the starting node
///
/// TODO: Implement graph traversal
pub async fn neighbors(
    _scope: GraphScope,
    _commit_id: String,
    _node_id: String,
    _depth: u32,
) -> ResultEnvelope<Vec<NodeSnapshot>> {
    let builder = ResultEnvelope::builder()
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Error,
            "neighbors not yet implemented - graph traversal layer pending".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    builder
        .error_with_code(
            "Graph traversal operations not yet implemented",
            "NOT_IMPLEMENTED",
        )
        .build()
        .expect("Valid envelope")
}

/// Determine whether a path exists between two nodes within the depth constraint
///
/// TODO: Implement graph path finding
pub async fn path_exists(
    _scope: GraphScope,
    _commit_id: String,
    _from: String,
    _to: String,
    _max_depth: u32,
) -> ResultEnvelope<bool> {
    let builder = ResultEnvelope::builder()
        .add_diagnostic(Diagnostic::new(
            DiagnosticLevel::Error,
            "path_exists not yet implemented - graph query layer pending".to_string(),
        ))
        .with_source_info("graph-capsule", Some("0.0.1"), None::<String>);

    builder
        .error_with_code("Graph path finding not yet implemented", "NOT_IMPLEMENTED")
        .build()
        .expect("Valid envelope")
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
