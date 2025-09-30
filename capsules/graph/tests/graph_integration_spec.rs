//! Integration tests for graph capsule with JetStream
//!
//! These tests verify:
//! - Commit operations emit events matching contract schemas
//! - Tag operations emit events and store in KV
//! - Events can be replayed from GRAPH_COMMITS stream
//! - Commit IDs are deterministic

use anyhow::Result;
use async_nats::jetstream::{self, consumer::DeliverPolicy};
use capsules_graph::{self as graph, GraphScope, Mutation, Property};
use futures_util::StreamExt;
use std::time::Duration;

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

#[tokio::test]
async fn given_graph_create_when_committed_then_event_appears_in_stream() -> Result<()> {
    // Arrange
    let scope = GraphScope {
        tenant_id: format!("tenant-{}", uuid::Uuid::new_v4()),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let mutations = vec![Mutation::AddNode {
        node_id: "root".to_string(),
        labels: vec!["Root".to_string()],
        properties: vec![Property {
            key: "name".to_string(),
            value: serde_json::json!("Root Node"),
        }],
    }];

    // Act - create graph
    let envelope = graph::create(scope.clone(), mutations.clone()).await;

    // Assert - envelope is success
    assert!(envelope.result.is_success());
    let commit_id = if let envelope::OperationResult::Success { data, .. } = &envelope.result {
        data.commit_id.clone()
    } else {
        panic!("Expected success result");
    };

    // Verify event in JetStream
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    // Wait a bit for event propagation
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await?;

    let mut batch = consumer
        .batch()
        .max_messages(10)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;

    let mut found_event = false;
    while let Some(msg) = batch.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;

        if event["commitId"] == commit_id {
            found_event = true;
            assert_eq!(event["event"], "graph.commit.created:v1");
            assert_eq!(event["graphId"], scope.graph_id);
            assert_eq!(event["tenantId"], scope.tenant_id);
            assert!(event["mutations"].is_array());
            assert_eq!(event["mutations"].as_array().unwrap().len(), 1);
        }
    }

    assert!(found_event, "Commit event should appear in stream");

    Ok(())
}

#[tokio::test]
async fn given_graph_commit_when_mutations_applied_then_deterministic_commit_id() -> Result<()> {
    // Arrange
    let scope = GraphScope {
        tenant_id: "tenant-determinism".to_string(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let mutations = vec![
        Mutation::AddNode {
            node_id: "node-a".to_string(),
            labels: vec!["Label1".to_string()],
            properties: vec![],
        },
        Mutation::AddNode {
            node_id: "node-b".to_string(),
            labels: vec!["Label2".to_string()],
            properties: vec![],
        },
    ];

    // Act - compute commit ID twice
    let commit_id_1 = graph::compute_commit_id(&scope, None, &mutations);
    let commit_id_2 = graph::compute_commit_id(&scope, None, &mutations);

    // Assert
    assert_eq!(commit_id_1, commit_id_2);
    assert_eq!(commit_id_1.len(), 64); // SHA256 hex

    Ok(())
}

#[tokio::test]
async fn given_tag_operation_when_set_then_event_emitted() -> Result<()> {
    // Arrange
    let scope = GraphScope {
        tenant_id: format!("tenant-tag-{}", uuid::Uuid::new_v4()),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let tag_name = "v1.0.0";
    let commit_id = "a".repeat(64); // Fake commit ID

    // Act
    let envelope = graph::tag(scope.clone(), tag_name.to_string(), commit_id.clone()).await;

    // Assert
    assert!(envelope.result.is_success());

    // Verify tag event in JetStream
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject.clone(),
            deliver_policy: DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await?;

    let mut batch = consumer
        .batch()
        .max_messages(10)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;

    let mut found_tag_event = false;
    while let Some(msg) = batch.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;

        if event["event"] == "graph.tag.updated:v1" && event["tag"] == tag_name {
            found_tag_event = true;
            assert_eq!(event["action"], "set");
            assert_eq!(event["commitId"], commit_id);
        }
    }

    assert!(found_tag_event, "Tag event should appear in stream");

    Ok(())
}

#[tokio::test]
async fn given_commit_with_parent_when_created_then_includes_parent_in_event() -> Result<()> {
    // Arrange
    let scope = GraphScope {
        tenant_id: format!("tenant-parent-{}", uuid::Uuid::new_v4()),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let parent_commit_id = "b".repeat(64);
    let mutations = vec![Mutation::AddNode {
        node_id: "child-node".to_string(),
        labels: vec![],
        properties: vec![],
    }];

    // Act
    let envelope = graph::commit(scope.clone(), Some(parent_commit_id.clone()), mutations).await;

    // Assert
    assert!(envelope.result.is_success());
    let commit_id = if let envelope::OperationResult::Success { data, .. } = &envelope.result {
        assert_eq!(data.parent_commit_id, Some(parent_commit_id.clone()));
        data.commit_id.clone()
    } else {
        panic!("Expected success result");
    };

    // Verify event has parentCommitId
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!(
        "demon.graph.v1.{}.{}.{}.commit",
        scope.tenant_id, scope.project_id, scope.namespace
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject,
            deliver_policy: DeliverPolicy::All,
            ack_policy: jetstream::consumer::AckPolicy::None,
            ..Default::default()
        })
        .await?;

    let mut batch = consumer
        .batch()
        .max_messages(10)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;

    let mut found_event = false;
    while let Some(msg) = batch.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;

        if event["commitId"] == commit_id {
            found_event = true;
            assert_eq!(event["parentCommitId"], parent_commit_id);
        }
    }

    assert!(found_event, "Commit event with parent should appear");

    Ok(())
}

#[tokio::test]
async fn given_empty_mutations_when_commit_then_returns_error() {
    // Arrange
    let scope = GraphScope {
        tenant_id: "tenant-empty".to_string(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    // Act
    let envelope = graph::commit(scope, None, vec![]).await;

    // Assert
    assert!(matches!(
        envelope.result,
        envelope::OperationResult::Error { .. }
    ));
    assert!(!envelope.diagnostics.is_empty());
}
