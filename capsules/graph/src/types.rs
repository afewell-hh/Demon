//! Type definitions matching demon-graph.wit interface

use serde::{Deserialize, Serialize};

/// Graph scope identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphScope {
    pub tenant_id: String,
    pub project_id: String,
    pub namespace: String,
    pub graph_id: String,
}

/// Key/value property pair
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Property {
    pub key: String,
    pub value: serde_json::Value,
}

/// Snapshot of a graph node at a commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeSnapshot {
    pub node_id: String,
    pub labels: Vec<String>,
    pub properties: Vec<Property>,
}

/// Snapshot of a graph edge at a commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeSnapshot {
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub label: Option<String>,
    pub properties: Vec<Property>,
}

/// Graph mutation operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Mutation {
    #[serde(rename = "add-node")]
    AddNode {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(default)]
        labels: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        properties: Vec<Property>,
    },
    #[serde(rename = "update-node")]
    UpdateNode {
        #[serde(rename = "nodeId")]
        node_id: String,
        labels: Vec<String>,
        properties: Vec<Property>,
    },
    #[serde(rename = "remove-node")]
    RemoveNode {
        #[serde(rename = "nodeId")]
        node_id: String,
    },
    #[serde(rename = "add-edge")]
    AddEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
        from: String,
        to: String,
        label: Option<String>,
        properties: Vec<Property>,
    },
    #[serde(rename = "update-edge")]
    UpdateEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
        from: String,
        to: String,
        label: Option<String>,
        properties: Vec<Property>,
    },
    #[serde(rename = "remove-edge")]
    RemoveEdge {
        #[serde(rename = "edgeId")]
        edge_id: String,
    },
}

/// Association between a tag and commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaggedCommit {
    pub tag: String,
    pub commit_id: String,
    pub timestamp: String,
}
