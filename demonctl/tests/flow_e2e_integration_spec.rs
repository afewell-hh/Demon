// End-to-end integration tests for Agent Flow API
//
// Journey: Agent Authors and Submits Flow Manifest via CLI/API
// Persona: LLM agent drafting a new workflow based on available contracts and submitting it for execution
//
// Given: Agent has access to the Demon API, contract registry, and `demonctl` CLI
// When: The agent exports an example ritual, modifies it, validates the manifest, and submits it via API
// Then: The agent can complete the full workflow end-to-end

use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Helper: Export ritual to flow manifest
fn export_ritual_to_json(ritual_name: &str, output: &std::path::Path) -> Result<()> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "export",
            "--ritual",
            ritual_name,
            "--output",
            &output.to_string_lossy(),
        ])
        .assert()
        .success();

    Ok(())
}

/// Helper: Validate manifest with dry-run
fn validate_manifest(manifest_path: &std::path::Path) -> Result<bool> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    let result = Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "import",
            "--file",
            &manifest_path.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .success();

    Ok(result.get_output().status.success())
}

#[test]
fn given_echo_ritual_when_agent_exports_then_valid_manifest_created() -> Result<()> {
    // Given: Echo ritual exists in examples/rituals/
    let temp = TempDir::new()?;
    let output = temp.path().join("echo_export.json");

    // When: Agent exports echo ritual
    export_ritual_to_json("echo", &output)?;

    // Then: Export produces valid manifest
    assert!(output.exists(), "Exported manifest file should exist");

    let content = fs::read_to_string(&output)?;
    let manifest: serde_json::Value = serde_json::from_str(&content)?;

    // Then: Manifest has correct schema version
    assert_eq!(manifest["schema_version"], "v1");

    // Then: Manifest has flow ID
    assert_eq!(manifest["metadata"]["flow_id"], "flow-echo-ritual");

    // Then: Manifest has nodes and edges
    assert_eq!(manifest["metadata"]["name"], "Echo Ritual");
    assert_eq!(manifest["metadata"]["created_by"], "demonctl-cli");
    assert!(!manifest["nodes"].as_array().unwrap().is_empty());
    assert!(manifest["edges"].is_array());

    Ok(())
}

#[test]
fn given_exported_manifest_when_agent_validates_then_validation_passes() -> Result<()> {
    // Given: Exported echo ritual manifest
    let temp = TempDir::new()?;
    let output = temp.path().join("echo_validate.json");
    export_ritual_to_json("echo", &output)?;

    // When: Agent validates manifest
    let is_valid = validate_manifest(&output)?;

    // Then: Validation passes
    assert!(is_valid, "Manifest validation should pass");

    Ok(())
}

#[test]
fn given_invalid_manifest_when_agent_validates_then_validation_fails() -> Result<()> {
    // Given: Invalid manifest (missing required fields)
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("invalid.json");

    fs::write(
        &invalid_manifest,
        r#"{"schema_version": "v1", "metadata": {"flow_id": ""}}"#,
    )?;

    // When: Agent attempts to validate
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    let _result = Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure();

    // Then: Validation fails with helpful error
    Ok(())
}

#[test]
fn given_hello_agent_manifest_when_agent_validates_then_all_fields_present() -> Result<()> {
    // Given: hello-agent.json example manifest exists
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();
    let manifest_path = project_root.join("examples/flows/hello-agent.json");

    // Then: Manifest file exists
    assert!(
        manifest_path.exists(),
        "hello-agent.json should exist in examples/flows/"
    );

    // When: Agent reads and parses manifest
    let content = fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&content)?;

    // Then: Schema version is v1
    assert_eq!(manifest["schema_version"], "v1");

    // Then: Metadata is complete
    assert_eq!(manifest["metadata"]["flow_id"], "hello-agent-001");
    assert_eq!(manifest["metadata"]["name"], "Hello Agent Flow");
    assert_eq!(manifest["metadata"]["created_by"], "claude-agent-demo");

    // Then: Nodes array contains all node types
    let nodes = manifest["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4, "Should have 4 nodes");

    let node_types: Vec<&str> = nodes.iter().map(|n| n["type"].as_str().unwrap()).collect();
    assert!(node_types.contains(&"trigger"));
    assert!(node_types.contains(&"capsule"));
    assert!(node_types.contains(&"approval"));
    assert!(node_types.contains(&"completion"));

    // Then: Edges connect nodes properly
    let edges = manifest["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 3, "Should have 3 edges");

    // Then: Provenance information is present
    assert!(manifest["provenance"]["agent_id"].is_string());
    assert!(manifest["provenance"]["generation_timestamp"].is_string());

    // When: Agent validates manifest
    let is_valid = validate_manifest(&manifest_path)?;

    // Then: Validation passes
    assert!(is_valid, "hello-agent.json should pass validation");

    Ok(())
}

#[test]
fn given_yaml_format_when_agent_exports_then_yaml_manifest_created() -> Result<()> {
    // Given: Echo ritual exists
    let temp = TempDir::new()?;
    let output = temp.path().join("echo_export.yaml");

    // When: Agent exports to YAML format
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "export",
            "--ritual",
            "echo",
            "--output",
            &output.to_string_lossy(),
        ])
        .assert()
        .success();

    // Then: YAML file is created
    assert!(output.exists());

    // Then: YAML can be parsed
    let content = fs::read_to_string(&output)?;
    let manifest: serde_yaml::Value = serde_yaml::from_str(&content)?;

    assert_eq!(manifest["schema_version"], "v1");
    assert_eq!(manifest["metadata"]["flow_id"], "flow-echo-ritual");

    Ok(())
}

#[test]
fn given_manifest_with_duplicate_node_ids_when_validated_then_fails() -> Result<()> {
    // Given: Manifest with duplicate node IDs
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("duplicate_nodes.json");

    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v1",
            "metadata": {
                "flow_id": "test-duplicate",
                "name": "Test",
                "created_by": "test"
            },
            "nodes": [
                {"node_id": "same", "type": "trigger", "config": {}},
                {"node_id": "same", "type": "completion", "config": {}}
            ],
            "edges": []
        }"#,
    )?;

    // When: Agent attempts to validate
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    // Then: Validation fails
    Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Duplicate node_id found: same"));

    Ok(())
}

#[test]
fn given_manifest_with_invalid_edge_when_validated_then_fails() -> Result<()> {
    // Given: Manifest with edge referencing non-existent node
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("invalid_edge.json");

    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v1",
            "metadata": {
                "flow_id": "test-edge",
                "name": "Test",
                "created_by": "test"
            },
            "nodes": [
                {"node_id": "start", "type": "trigger", "config": {}}
            ],
            "edges": [
                {"from": "start", "to": "nonexistent"}
            ]
        }"#,
    )?;

    // When: Agent attempts to validate
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();

    // Then: Validation fails
    Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Edge references non-existent node: nonexistent",
        ));

    Ok(())
}

#[test]
fn given_cli_help_when_agent_queries_then_flow_commands_documented() {
    // When: Agent queries help
    Command::cargo_bin("demonctl")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Flow export/import commands"));

    // When: Agent queries flow export help
    Command::cargo_bin("demonctl")
        .unwrap()
        .args(["flow", "export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Export a ritual as a flow manifest",
        ))
        .stdout(predicate::str::contains("--ritual"))
        .stdout(predicate::str::contains("--output"));

    // When: Agent queries flow import help
    Command::cargo_bin("demonctl")
        .unwrap()
        .args(["flow", "import", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Import and optionally submit a flow manifest",
        ))
        .stdout(predicate::str::contains("--file"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--api-url"))
        .stdout(predicate::str::contains("--jwt"));
}

// Note: Full API submission tests require a running Operate UI instance with agent-flows feature flag
// These tests are marked as #[ignore] and should be run manually or in CI with proper setup

#[test]
#[ignore] // Requires Operate UI with agent-flows feature flag and NATS
fn given_operate_ui_running_when_agent_submits_flow_then_submission_succeeds() -> Result<()> {
    // This test would be similar to the operate-ui/tests/agent_flows_spec.rs tests
    // but exercised via demonctl CLI instead of direct API calls
    //
    // Prerequisites:
    // - NATS running
    // - Operate UI with OPERATE_UI_FLAGS=agent-flows
    // - JWT_SECRET configured
    // - Test JWT with flows:write scope
    //
    // Given: Exported flow manifest
    // When: demonctl flow import --file manifest.json --api-url http://localhost:3000 --jwt $TOKEN
    // Then: Flow is submitted successfully and appears in /api/flows

    Ok(())
}

#[test]
#[ignore] // Requires Operate UI with contracts-browser feature flag
fn given_operate_ui_running_when_agent_queries_contracts_then_contracts_returned() -> Result<()> {
    // This test validates the contracts discovery journey
    //
    // Prerequisites:
    // - NATS running
    // - Operate UI with OPERATE_UI_FLAGS=contracts-browser
    // - Schema registry available
    //
    // Given: Contract registry has contracts
    // When: Agent queries /api/contracts/registry/list
    // Then: Contracts are returned with metadata

    Ok(())
}
