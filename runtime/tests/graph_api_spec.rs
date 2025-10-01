//! Integration tests for graph REST API endpoints
//!
//! Tests verify that the REST server correctly exposes graph commit and tag queries.

use anyhow::Result;
use capsules_graph::GraphScope;
use envelope::OperationResult;
use serial_test::serial;
use std::time::Duration;

/// Helper to start the REST API server in background for testing
async fn start_test_server() -> Result<tokio::task::JoinHandle<()>> {
    let handle = tokio::spawn(async {
        let addr = ([127, 0, 0, 1], 18080).into();
        if let Err(e) = runtime::server::serve(addr).await {
            eprintln!("Server error: {}", e);
        }
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(200)).await;

    Ok(handle)
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:18080/api/graph/commits/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        commit_id, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:18080/api/graph/commits/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        commit_id, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:18080/api/graph/tags/{}?tenantId={}&projectId={}&namespace={}&graphId={}",
        tag, scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:18080/api/graph/tags/nonexistent?tenantId={}&projectId={}&namespace={}&graphId={}",
        scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let url = format!(
        "http://localhost:18080/api/graph/commits?tenantId={}&projectId={}&namespace={}&graphId={}",
        scope.tenant_id, scope.project_id, scope.namespace, scope.graph_id
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
    let _server = start_test_server().await?;

    let client = reqwest::Client::new();
    let response = client.get("http://localhost:18080/health").send().await?;

    // Assert
    assert_eq!(response.status(), 200);
    let body = response.text().await?;
    assert_eq!(body, "OK");

    Ok(())
}
