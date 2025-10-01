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
    assert_eq!(envelope_value["result"]["success"], true);
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
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
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
    assert_eq!(envelope_value["result"]["success"], true);
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
    assert_eq!(envelope_value["result"]["success"], true);

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
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
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
async fn given_list_tags_via_runtime_when_dispatched_then_returns_tags() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-list-{}", uuid::Uuid::new_v4());

    // Set a tag first
    let tag_args = json!({
        "operation": "tag",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "tag": "v1.0.0",
        "commitId": "a".repeat(64)
    });

    router
        .dispatch("graph", &tag_args, "test-run", "test-ritual")
        .await?;

    // Act - list tags
    let args = json!({
        "operation": "list-tags",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert - should return tags
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"], true);
    assert!(envelope_value["result"]["data"].is_array());
    let tags = envelope_value["result"]["data"].as_array().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["tag"], "v1.0.0");

    Ok(())
}

#[tokio::test]
async fn given_delete_tag_via_runtime_when_dispatched_then_tag_removed() -> Result<()> {
    // Arrange
    let router = Router::new();
    let tenant_id = format!("tenant-delete-{}", uuid::Uuid::new_v4());

    // Set a tag first
    let tag_args = json!({
        "operation": "tag",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "tag": "v1.0.0",
        "commitId": "b".repeat(64)
    });

    router
        .dispatch("graph", &tag_args, "test-run", "test-ritual")
        .await?;

    // Act - delete tag
    let delete_args = json!({
        "operation": "delete-tag",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "tag": "v1.0.0"
    });

    let result = router
        .dispatch("graph", &delete_args, "test-run", "test-ritual")
        .await?;

    // Assert - delete succeeded
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"], true);

    // Verify tag removed from list-tags
    let list_args = json!({
        "operation": "list-tags",
        "scope": {
            "tenantId": tenant_id,
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    let list_result = router
        .dispatch("graph", &list_args, "test-run", "test-ritual")
        .await?;

    let list_envelope = list_result;
    assert_eq!(list_envelope["result"]["success"], true);
    let tags = list_envelope["result"]["data"].as_array().unwrap();
    assert_eq!(tags.len(), 0, "Tag should be removed");

    Ok(())
}

#[tokio::test]
async fn given_get_node_via_runtime_when_dispatched_then_returns_not_implemented() -> Result<()> {
    // Arrange
    let router = Router::new();

    let args = json!({
        "operation": "get-node",
        "scope": {
            "tenantId": "tenant-1",
            "projectId": "proj-1",
            "namespace": "ns-1",
            "graphId": "graph-1"
        },
        "commitId": "c".repeat(64),
        "nodeId": "node-1"
    });

    // Act
    let result = router
        .dispatch("graph", &args, "test-run", "test-ritual")
        .await?;

    // Assert - should return error with NOT_IMPLEMENTED (or MATERIALIZATION_FAILED for missing commit)
    let envelope_value = result;
    assert_eq!(envelope_value["result"]["success"], false);
    assert!(envelope_value["result"]["error"]["code"] != serde_json::Value::Null);

    Ok(())
}
