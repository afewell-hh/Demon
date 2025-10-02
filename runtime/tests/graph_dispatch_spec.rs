//! Runtime integration tests for graph capsule dispatch
//!
//! These tests verify that the runtime router can dispatch graph operations
//! and that events are emitted to JetStream.

use anyhow::Result;
use async_nats::jetstream;
use futures_util::StreamExt;
use runtime::link::router::Router;
use serde_json::json;
use std::time::Duration;

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

#[tokio::test]
#[ignore]
async fn given_graph_create_via_runtime_when_dispatched_then_commit_event_emitted() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-{}", uuid::Uuid::new_v4());

    let args = json!({
        "operation": "create",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "seed": [
            {
                "op": "add-node",
                "nodeId": "root",
                "labels": ["Root"],
                "properties": []
            }
        ]
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert - envelope is success
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"].as_bool(), Some(true));
    let commit_id = envelope_value["result"]["data"]["commit_id"]
        .as_str()
        .expect("Should have commit ID");

    // Verify event in JetStream
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!("demon.graph.v1.{}.proj-1.ns-1.commit", tenant_id);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject,
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
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
        let msg = msg.map_err(|e| anyhow::anyhow!("Failed to get message: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;

        if event["commitId"] == commit_id {
            found_event = true;
            assert_eq!(event["event"], "graph.commit.created:v1");
            assert_eq!(event["graphId"], "graph-1");
            assert_eq!(event["tenantId"], tenant_id);
        }
    }

    assert!(found_event, "Commit event should appear in stream");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_graph_commit_via_runtime_when_dispatched_then_event_has_parent() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-{}", uuid::Uuid::new_v4());
    let parent_commit_id = "a".repeat(64);

    let args = json!({
        "operation": "commit",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "parentRef": parent_commit_id,
        "mutations": [
            {
                "op": "add-node",
                "nodeId": "child",
                "labels": [],
                "properties": []
            }
        ]
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"].as_bool(), Some(true));
    let commit_id = envelope_value["result"]["data"]["commit_id"]
        .as_str()
        .expect("Should have commit ID");
    let returned_parent = envelope_value["result"]["data"]["parent_commit_id"]
        .as_str()
        .expect("Should have parent commit ID");
    assert_eq!(returned_parent, parent_commit_id);

    // Verify event has parentCommitId
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!("demon.graph.v1.{}.proj-1.ns-1.commit", tenant_id);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject,
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
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
        let msg = msg.map_err(|e| anyhow::anyhow!("Failed to get message: {}", e))?;
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
#[ignore]
async fn given_graph_tag_via_runtime_when_dispatched_then_tag_event_emitted() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-{}", uuid::Uuid::new_v4());
    let commit_id = "b".repeat(64);
    let tag = "v1.0.0";

    let args = json!({
        "operation": "tag",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "tag": tag,
        "commitId": commit_id
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"].as_bool(), Some(true));

    // Verify tag event in JetStream
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let subject = format!("demon.graph.v1.{}.proj-1.ns-1.commit", tenant_id);

    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = js.get_stream("GRAPH_COMMITS").await?;
    let consumer = stream
        .create_consumer(jetstream::consumer::pull::Config {
            filter_subject: subject,
            deliver_policy: jetstream::consumer::DeliverPolicy::All,
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
        let msg = msg.map_err(|e| anyhow::anyhow!("Failed to get message: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;

        if event["event"] == "graph.tag.updated:v1" && event["tag"] == tag {
            found_event = true;
            assert_eq!(event["action"], "set");
            assert_eq!(event["commitId"], commit_id);
        }
    }

    assert!(found_event, "Tag event should appear in stream");

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_list_tags_via_runtime_when_dispatched_then_returns_placeholder() -> Result<()> {
    // Arrange
    let router = Router::new();

    let args = json!({
        "operation": "list-tags",
        "scope": {
            "tenantId": "tenant-1",
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        }
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert - should succeed but return empty (placeholder)
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"].as_bool(), Some(true));
    assert!(envelope_value["result"]["data"].is_array());

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_get_node_via_runtime_when_dispatched_then_returns_node_snapshot() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-{}", uuid::Uuid::new_v4());

    let create_args = json!({
        "operation": "create",
        "scope": {
            "tenantId": &tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "seed": [
            {
                "op": "add-node",
                "nodeId": "root",
                "labels": ["Root"],
                "properties": []
            }
        ]
    });

    let create_result = router
        .dispatch("graph", &create_args, "test-run", "test-ritual")
        .await?;
    let commit_id = create_result["result"]["data"]["commit_id"]
        .as_str()
        .expect("commit id present")
        .to_string();

    let args = json!({
        "operation": "get-node",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "commitId": commit_id,
        "nodeId": "root"
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert - should succeed and return node snapshot
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"].as_bool(), Some(true));
    let node = envelope_value["result"]["data"]
        .as_object()
        .expect("node snapshot");
    assert_eq!(node["nodeId"], "root");

    Ok(())
}
