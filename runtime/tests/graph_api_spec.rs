//! Integration tests for graph REST API endpoints
//!
//! Tests verify that the REST server correctly exposes graph commit and tag queries.

use anyhow::{Context, Result};
use capsules_graph::GraphScope;
use envelope::OperationResult;
use serial_test::serial;
use std::{net::SocketAddr, time::Duration};
use tokio::task::JoinHandle;

/// Handle to the background test server. Aborts on drop to avoid leaking tasks.
struct TestServer {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl TestServer {
    fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Helper to start the REST API server in background for testing
async fn start_test_server() -> Result<TestServer> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .context("failed to bind ephemeral port for test server")?;
    let addr = listener
        .local_addr()
        .context("failed to read bound address for test server")?;
    drop(listener);

    let handle = tokio::spawn(async move {
        if let Err(e) = runtime::server::serve(addr).await {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(TestServer { addr, handle })
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_commit_exists_when_get_commit_then_returns_commit_data() -> Result<()> {
    // Arrange - create a commit via capsule
    let tenant_id = format!("tenant-api-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let mutations = vec![capsules_graph::Mutation::AddNode {
        node_id: "node-1".to_string(),
        labels: vec!["Test".to_string()],
        properties: vec![],
    }];

    let envelope = capsules_graph::create(scope.clone(), mutations).await;
    assert!(envelope.result.is_success());
    let commit_id = match envelope.result {
        OperationResult::Success { data, .. } => data.commit_id,
        _ => panic!("Expected success result"),
    };

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Act - start server and query commit
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/commits/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, commit_id, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await?;

    assert_eq!(body["commitId"], commit_id);
    assert_eq!(body["event"], "graph.commit.created:v1");
    assert_eq!(body["tenantId"], tenant_id);

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_commit_missing_when_get_commit_then_returns_404() -> Result<()> {
    // Arrange
    let tenant_id = format!("tenant-404-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let commit_id = "nonexistent".repeat(8); // 64 chars

    // Act
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/commits/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, commit_id, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert
    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["code"], "COMMIT_NOT_FOUND");

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_tag_exists_when_get_tag_then_returns_tag_data() -> Result<()> {
    // Arrange - create a tag via capsule
    let tenant_id = format!("tenant-tag-api-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    let commit_id = "a".repeat(64);
    let tag = "v1.0.0";

    let envelope = capsules_graph::tag(scope.clone(), tag.to_string(), commit_id.clone()).await;
    assert!(envelope.result.is_success());

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Act - query tag via REST API
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/tags/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, tag, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert
    assert_eq!(response.status(), 200);

    // Verify ETag header before consuming response
    let etag = response.headers().get("etag");
    assert!(etag.is_some(), "ETag header should be present");

    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["tag"], tag);
    assert_eq!(body["commitId"], commit_id);

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_tag_missing_when_get_tag_then_returns_404() -> Result<()> {
    // Arrange
    let tenant_id = format!("tenant-tag-404-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    // Act
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/tags/nonexistent?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert
    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["code"], "TAG_NOT_FOUND");

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_commits_exist_when_list_commits_then_returns_array() -> Result<()> {
    // Arrange - create multiple commits
    let tenant_id = format!("tenant-list-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    // Create initial commit
    let envelope1 = capsules_graph::create(
        scope.clone(),
        vec![capsules_graph::Mutation::AddNode {
            node_id: "node-1".to_string(),
            labels: vec![],
            properties: vec![],
        }],
    )
    .await;
    assert!(envelope1.result.is_success());

    // Create second commit
    let parent = match envelope1.result {
        OperationResult::Success { data, .. } => data.commit_id,
        _ => panic!("Expected success result"),
    };
    let envelope2 = capsules_graph::commit(
        scope.clone(),
        Some(parent.clone()),
        vec![capsules_graph::Mutation::AddNode {
            node_id: "node-2".to_string(),
            labels: vec![],
            properties: vec![],
        }],
    )
    .await;
    assert!(envelope2.result.is_success());

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Act - list commits via REST API
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/commits?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await?;

    assert!(body.is_array());
    let commits = body.as_array().unwrap();
    assert!(commits.len() >= 2, "Should have at least 2 commits");

    Ok(())
}

#[tokio::test]
#[serial]
async fn given_health_endpoint_when_requested_then_returns_ok() -> Result<()> {
    // Act
    let server = start_test_server().await?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/health", server.addr()))
        .send()
        .await?;

    // Assert
    assert_eq!(response.status(), 200);
    let body = response.text().await?;
    assert_eq!(body, "OK");

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore] // Requires NATS; run via CI with --ignored
async fn given_sse_endpoint_when_connected_then_receives_init_and_heartbeats() -> Result<()> {
    use futures_util::StreamExt;

    // Arrange - create a scope with commits
    let tenant_id = format!("tenant-sse-{}", uuid::Uuid::new_v4());
    let scope = GraphScope {
        tenant_id: tenant_id.clone(),
        project_id: "proj-1".to_string(),
        namespace: "ns-1".to_string(),
        graph_id: "graph-1".to_string(),
    };

    // Create a commit so we have data to stream
    let envelope = capsules_graph::create(
        scope.clone(),
        vec![capsules_graph::Mutation::AddNode {
            node_id: "node-1".to_string(),
            labels: vec!["Test".to_string()],
            properties: vec![],
        }],
    )
    .await;
    assert!(envelope.result.is_success());

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Act - start server and connect to SSE endpoint
    let server = start_test_server().await?;
    std::env::set_var("SSE_HEARTBEAT_SECONDS", "1"); // Fast heartbeat for test

    let client = reqwest::Client::new();
    let base_url = format!("http://{}", server.addr());
    let url = format!(
        "{}/api/graph/commits/stream?tenantId={}&projectId={}&namespace={}&graphId={}",
        base_url, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
    );

    let response = client.get(&url).send().await?;

    // Assert - SSE headers
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-cache");
    assert_eq!(response.headers().get("connection").unwrap(), "keep-alive");

    // Read first few events from stream
    let mut stream = response.bytes_stream();
    let mut events = Vec::new();
    let mut event_count = 0;

    while event_count < 3 {
        tokio::select! {
            Some(Ok(chunk)) = stream.next() => {
                let text = String::from_utf8_lossy(&chunk);
                if text.contains("event:") {
                    events.push(text.to_string());
                    event_count += 1;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(3)) => {
                break; // Timeout after 3s
            }
        }
    }

    // Assert - should receive init event and at least one heartbeat
    let all_events = events.join("");
    assert!(
        all_events.contains("event: init"),
        "Should receive init event"
    );
    assert!(
        all_events.contains("event: heartbeat"),
        "Should receive heartbeat events"
    );

    // Parse init event and verify structure
    let init_event = events
        .iter()
        .find(|e| e.contains("event: init"))
        .expect("Should have init event");

    assert!(
        init_event.contains(&format!("\"tenantId\":\"{}\"", tenant_id)),
        "Init event should contain tenantId"
    );

    Ok(())
}
