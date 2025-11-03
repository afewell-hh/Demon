//! Integration tests for JetStream KV client
//!
//! These tests require a running NATS server with JetStream enabled.
//! Run with: NATS_URL=nats://127.0.0.1:4222 cargo test -p demon-registry -- --nocapture

use anyhow::Result;
use demon_registry::kv::{ContractBundle, KvClient};

async fn new_isolated_client() -> Result<(KvClient, String)> {
    let url = nats_url();
    std::env::set_var("NATS_URL", &url);

    let bucket = format!("contracts_test_{}", uuid::Uuid::new_v4());
    std::env::set_var("REGISTRY_KV_BUCKET", &bucket);

    let client = KvClient::new().await?;
    Ok((client, bucket))
}

fn nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_empty_kv_when_listing_contracts_then_returns_empty_array() -> Result<()> {
    // Arrange
    let (client, bucket) = new_isolated_client().await?;

    // Act
    let contracts = client.list_contracts().await?;

    // Assert
    assert!(
        contracts.is_empty(),
        "Expected empty KV bucket {}, found {} items",
        bucket,
        contracts.len()
    );

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_contract_when_stored_and_retrieved_then_data_matches() -> Result<()> {
    // Arrange
    let (client, _) = new_isolated_client().await?;
    let test_name = format!("test-contract-{}", uuid::Uuid::new_v4());

    let bundle = ContractBundle {
        name: test_name.clone(),
        version: "1.0.0".to_string(),
        description: Some("Test contract for integration test".to_string()),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        json_schema: Some(r#"{"type": "object"}"#.to_string()),
        wit_path: Some("/contracts/test.wit".to_string()),
        descriptor_path: Some("/contracts/test.json".to_string()),
        digest: Some("abc123".to_string()),
    };

    // Act - Store the contract
    client.put_contract(&bundle).await?;

    // Retrieve the contract
    let retrieved = client
        .get_contract(&test_name, "1.0.0")
        .await?
        .expect("Contract should exist");

    // Assert
    assert_eq!(retrieved.name, test_name);
    assert_eq!(retrieved.version, "1.0.0");
    assert_eq!(retrieved.description, bundle.description);
    assert_eq!(retrieved.json_schema, bundle.json_schema);
    assert_eq!(retrieved.wit_path, bundle.wit_path);
    assert_eq!(retrieved.descriptor_path, bundle.descriptor_path);

    // Cleanup
    client.delete_contract(&test_name, "1.0.0").await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_stored_contract_when_listed_then_appears_in_results() -> Result<()> {
    // Arrange
    let (client, _) = new_isolated_client().await?;
    let test_name = format!("list-test-{}", uuid::Uuid::new_v4());

    let bundle = ContractBundle {
        name: test_name.clone(),
        version: "2.0.0".to_string(),
        description: Some("List test contract".to_string()),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        json_schema: None,
        wit_path: None,
        descriptor_path: None,
        digest: Some("def456".to_string()),
    };

    // Act
    client.put_contract(&bundle).await?;

    let contracts = client.list_contracts().await?;

    // Assert
    let found = contracts
        .iter()
        .any(|c| c.name == test_name && c.version == "2.0.0");
    assert!(found, "Contract should appear in list");

    // Cleanup
    client.delete_contract(&test_name, "2.0.0").await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS server running
async fn given_nonexistent_contract_when_retrieved_then_returns_none() -> Result<()> {
    // Arrange
    let (client, _) = new_isolated_client().await?;

    // Act
    let result = client
        .get_contract("nonexistent-contract", "99.99.99")
        .await?;

    // Assert
    assert!(result.is_none(), "Nonexistent contract should return None");

    Ok(())
}
