//! Integration tests for graph JetStream storage resources
//!
//! These tests verify that:
//! - GRAPH_COMMITS stream is created with correct configuration
//! - GRAPH_TAGS KV bucket is created and supports read/write
//! - Stream idempotency (calling ensure_graph_stream multiple times)
//! - Replay capability (can fetch commits from stream)

use anyhow::Result;
use async_nats::jetstream::{self, consumer::DeliverPolicy, stream::RetentionPolicy};
use futures_util::StreamExt;
use serde_json::json;
use std::time::Duration;

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

#[tokio::test]
#[ignore]
async fn given_jetstream_when_ensure_stream_then_creates_with_expected_config() -> Result<()> {
    // Arrange
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Act
    let stream_info = engine::graph::ensure_graph_stream(&js, None).await?;

    // Assert - verify stream config
    assert_eq!(stream_info.config.name, "GRAPH_COMMITS");
    assert_eq!(
        stream_info.config.subjects,
        vec!["demon.graph.v1.*.*.*.commit"]
    );
    assert_eq!(
        stream_info.config.retention,
        RetentionPolicy::Limits,
        "should use Limits retention for replay"
    );
    assert_eq!(
        stream_info.config.max_messages_per_subject, 10_000,
        "should cap per-subject messages for backpressure"
    );
    assert_eq!(
        stream_info.config.duplicate_window,
        Duration::from_secs(120),
        "should prevent duplicates within 120s"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_existing_stream_when_ensure_called_twice_then_idempotent() -> Result<()> {
    // Arrange
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Act - call twice
    let info1 = engine::graph::ensure_graph_stream(&js, None).await?;
    let info2 = engine::graph::ensure_graph_stream(&js, None).await?;

    // Assert - both succeed and return same stream
    assert_eq!(info1.config.name, info2.config.name);
    assert_eq!(info1.config.subjects, info2.config.subjects);

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_jetstream_when_ensure_tag_store_then_creates_kv_bucket() -> Result<()> {
    // Arrange
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Act
    let store = engine::graph::ensure_graph_tag_store(&js, None).await?;

    // Assert - verify we can write and read tags
    let tag_key = format!("test-tag-{}", uuid::Uuid::new_v4());
    let tag_value = "commit-abc123";

    store.put(&tag_key, tag_value.into()).await?;
    let entry = store.get(&tag_key).await?;
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().as_ref(), tag_value.as_bytes());

    // Cleanup
    store.delete(&tag_key).await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_jetstream_when_ensure_graph_storage_then_creates_both_resources() -> Result<()> {
    // Arrange
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    // Act
    let (stream_info, tag_store) = engine::graph::ensure_graph_storage(&js, None).await?;

    // Assert stream
    assert_eq!(stream_info.config.name, "GRAPH_COMMITS");

    // Assert tag store works
    let test_key = format!("orchestration-test-{}", uuid::Uuid::new_v4());
    tag_store.put(&test_key, "test-commit".into()).await?;
    let entry = tag_store.get(&test_key).await?;
    assert!(entry.is_some());
    tag_store.delete(&test_key).await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_commit_published_when_replay_consumer_created_then_fetches_all_commits() -> Result<()>
{
    // Arrange - ensure stream and publish a commit event
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client.clone());
    engine::graph::ensure_graph_stream(&js, None).await?;

    let tenant = "tenant-replay";
    let project = "proj-replay";
    let namespace = "ns-replay";
    let commit_id = uuid::Uuid::new_v4().to_string();

    let subject = format!("demon.graph.v1.{}.{}.{}.commit", tenant, project, namespace);

    let commit_event = json!({
        "event": "graph.commit.created:v1",
        "graphId": "test-graph",
        "tenantId": tenant,
        "projectId": project,
        "namespace": namespace,
        "commitId": commit_id,
        "ts": chrono::Utc::now().to_rfc3339(),
        "mutations": []
    });

    // Act - publish commit
    let ack = js
        .publish(subject.clone(), serde_json::to_vec(&commit_event)?.into())
        .await?;
    ack.await?; // wait for ack

    // Create replay consumer (DeliverPolicy::All fetches from beginning)
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
        .max_messages(100)
        .expires(Duration::from_secs(2))
        .messages()
        .await?;

    // Assert - fetch and verify commit
    let mut found_commits = vec![];
    while let Some(msg) = batch.next().await {
        let msg = msg.map_err(|e| anyhow::anyhow!("batch error: {}", e))?;
        let event: serde_json::Value = serde_json::from_slice(&msg.message.payload)?;
        found_commits.push(event);
    }

    assert!(
        found_commits.iter().any(|e| e["commitId"] == commit_id),
        "should find our published commit via replay consumer"
    );

    Ok(())
}

#[tokio::test]
#[ignore]
async fn given_custom_config_when_ensure_stream_then_respects_overrides() -> Result<()> {
    // Arrange
    let client = async_nats::connect(&nats_url()).await?;
    let js = jetstream::new(client);

    let custom_config = engine::graph::GraphStorageConfig {
        stream_name: Some("TEST_GRAPH_COMMITS".to_string()),
        tag_bucket_name: None,
        subject_prefix: Some("test.graph.v1".to_string()),
    };

    // Act
    let stream_info = engine::graph::ensure_graph_stream(&js, Some(&custom_config)).await?;

    // Assert
    assert_eq!(stream_info.config.name, "TEST_GRAPH_COMMITS");
    assert_eq!(
        stream_info.config.subjects,
        vec!["test.graph.v1.*.*.*.commit"]
    );

    // Cleanup
    js.delete_stream("TEST_GRAPH_COMMITS").await?;

    Ok(())
}
