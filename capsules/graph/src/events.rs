//! Event emission for graph operations
//!
//! Emits graph.commit.created:v1 and graph.tag.updated:v1 events to
//! NATS JetStream following the contract schemas.

use crate::types::{GraphScope, Mutation};
use crate::TagAction;
use anyhow::{Context, Result};
use async_nats::jetstream;
use chrono::{DateTime, Utc};

/// Emit graph.commit.created:v1 event to JetStream
///
/// Subject: demon.graph.v1.{tenant}.{project}.{namespace}.commit
/// Idempotency: Nats-Msg-Id = "{tenant}:{project}:{namespace}:{commitId}"
pub async fn emit_commit_created(
    scope: &GraphScope,
    commit_id: &str,
    parent_commit_id: Option<&str>,
    mutations: &[Mutation],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client);

    // Ensure stream exists (idempotent)
    // Note: In production, this would be done at startup, not per-event
    let _ = ensure_graph_stream(&js).await;

    // Build event payload matching schema
    let mut payload = serde_json::json!({
        "event": "graph.commit.created:v1",
        "graphId": scope.graph_id,
        "tenantId": scope.tenant_id,
        "projectId": scope.project_id,
        "namespace": scope.namespace,
        "commitId": commit_id,
        "ts": timestamp.to_rfc3339(),
        "mutations": serialize_mutations(mutations),
    });

    if let Some(parent) = parent_commit_id {
        payload
            .as_object_mut()
            .unwrap()
            .insert("parentCommitId".to_string(), serde_json::json!(parent));
    }

    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!(
        "{}:{}:{}:{}",
        scope.tenant_id, scope.project_id, scope.namespace, commit_id
    );
    headers.insert("Nats-Msg-Id", msg_id.as_str());

    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await
        .context("Failed to await JetStream ack for commit event")?;

    Ok(())
}

/// Emit graph.tag.updated:v1 event to JetStream
///
/// Subject: demon.graph.v1.{tenant}.{project}.{namespace}.tag
/// Idempotency: Nats-Msg-Id = "{tenant}:{project}:{namespace}:tag:{tag}"
pub async fn emit_tag_updated(
    scope: &GraphScope,
    tag: &str,
    commit_id: Option<&str>,
    action: TagAction,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let client = async_nats::connect(&url).await?;
    let js = jetstream::new(client);

    // Ensure stream exists (idempotent)
    let _ = ensure_graph_stream(&js).await;

    // Build event payload matching schema
    let action_str = match action {
        TagAction::Set => "set",
        TagAction::Delete => "delete",
    };

    let mut payload = serde_json::json!({
        "event": "graph.tag.updated:v1",
        "graphId": scope.graph_id,
        "tenantId": scope.tenant_id,
        "projectId": scope.project_id,
        "namespace": scope.namespace,
        "tag": tag,
        "ts": timestamp.to_rfc3339(),
        "action": action_str,
    });

    if let Some(cid) = commit_id {
        payload
            .as_object_mut()
            .unwrap()
            .insert("commitId".to_string(), serde_json::json!(cid));
    }

    // Subject for tag events (using commit subject for now - could be separate)
    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    let mut headers = async_nats::HeaderMap::new();
    let msg_id = format!(
        "{}:{}:{}:tag:{}",
        scope.tenant_id, scope.project_id, scope.namespace, tag
    );
    headers.insert("Nats-Msg-Id", msg_id.as_str());

    js.publish_with_headers(subject, headers, serde_json::to_vec(&payload)?.into())
        .await?
        .await
        .context("Failed to await JetStream ack for tag event")?;

    Ok(())
}

/// Serialize mutations to JSON format matching contract schema
fn serialize_mutations(mutations: &[Mutation]) -> Vec<serde_json::Value> {
    mutations
        .iter()
        .map(|m| match m {
            Mutation::AddNode {
                node_id,
                labels,
                properties,
            } => {
                let mut obj = serde_json::json!({
                    "op": "add-node",
                    "nodeId": node_id,
                    "labels": labels,
                });
                if !properties.is_empty() {
                    let props: serde_json::Map<String, serde_json::Value> = properties
                        .iter()
                        .map(|p| (p.key.clone(), p.value.clone()))
                        .collect();
                    obj.as_object_mut()
                        .unwrap()
                        .insert("properties".to_string(), serde_json::Value::Object(props));
                }
                obj
            }
            Mutation::UpdateNode {
                node_id,
                labels,
                properties,
            } => {
                let mut obj = serde_json::json!({
                    "op": "update-node",
                    "nodeId": node_id,
                    "labels": labels,
                });
                if !properties.is_empty() {
                    let props: serde_json::Map<String, serde_json::Value> = properties
                        .iter()
                        .map(|p| (p.key.clone(), p.value.clone()))
                        .collect();
                    obj.as_object_mut()
                        .unwrap()
                        .insert("properties".to_string(), serde_json::Value::Object(props));
                }
                obj
            }
            Mutation::RemoveNode { node_id } => {
                serde_json::json!({
                    "op": "remove-node",
                    "nodeId": node_id,
                })
            }
            Mutation::AddEdge {
                edge_id,
                from,
                to,
                label,
                properties,
            } => {
                let mut obj = serde_json::json!({
                    "op": "add-edge",
                    "edgeId": edge_id,
                    "from": from,
                    "to": to,
                });
                if let Some(l) = label {
                    obj.as_object_mut()
                        .unwrap()
                        .insert("label".to_string(), serde_json::json!(l));
                }
                if !properties.is_empty() {
                    let props: serde_json::Map<String, serde_json::Value> = properties
                        .iter()
                        .map(|p| (p.key.clone(), p.value.clone()))
                        .collect();
                    obj.as_object_mut()
                        .unwrap()
                        .insert("properties".to_string(), serde_json::Value::Object(props));
                }
                obj
            }
            Mutation::UpdateEdge {
                edge_id,
                from,
                to,
                label,
                properties,
            } => {
                let mut obj = serde_json::json!({
                    "op": "update-edge",
                    "edgeId": edge_id,
                    "from": from,
                    "to": to,
                });
                if let Some(l) = label {
                    obj.as_object_mut()
                        .unwrap()
                        .insert("label".to_string(), serde_json::json!(l));
                }
                if !properties.is_empty() {
                    let props: serde_json::Map<String, serde_json::Value> = properties
                        .iter()
                        .map(|p| (p.key.clone(), p.value.clone()))
                        .collect();
                    obj.as_object_mut()
                        .unwrap()
                        .insert("properties".to_string(), serde_json::Value::Object(props));
                }
                obj
            }
            Mutation::RemoveEdge { edge_id } => {
                serde_json::json!({
                    "op": "remove-edge",
                    "edgeId": edge_id,
                })
            }
        })
        .collect()
}

/// Ensure GRAPH_COMMITS stream exists
async fn ensure_graph_stream(js: &jetstream::Context) -> Result<()> {
    let stream_config = jetstream::stream::Config {
        name: "GRAPH_COMMITS".to_string(),
        subjects: vec!["demon.graph.v1.*.*.*.commit".to_string()],
        retention: jetstream::stream::RetentionPolicy::Limits,
        storage: jetstream::stream::StorageType::File,
        max_messages_per_subject: 10_000,
        discard: jetstream::stream::DiscardPolicy::Old,
        duplicate_window: std::time::Duration::from_secs(120),
        ..Default::default()
    };

    js.get_or_create_stream(stream_config)
        .await
        .context("Failed to ensure GRAPH_COMMITS stream")?;

    Ok(())
}
