use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn export_echo_ritual_produces_valid_manifest() -> Result<()> {
    let temp = TempDir::new()?;
    let output = temp.path().join("echo_flow.json");

    // Get absolute path to project root (parent of demonctl/)
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
        .success()
        .stdout(predicate::str::contains("✓ Exported flow manifest"))
        .stdout(predicate::str::contains("Flow ID: flow-echo-ritual"))
        .stdout(predicate::str::contains("Nodes: 3"))
        .stdout(predicate::str::contains("Edges: 2"));

    // Verify the file was created
    assert!(output.exists());

    // Verify the file can be parsed as JSON
    let content = fs::read_to_string(&output)?;
    let manifest: serde_json::Value = serde_json::from_str(&content)?;

    // Verify key fields
    assert_eq!(manifest["schema_version"], "v1");
    assert_eq!(manifest["metadata"]["flow_id"], "flow-echo-ritual");
    assert_eq!(manifest["metadata"]["name"], "Echo Ritual");
    assert_eq!(manifest["metadata"]["created_by"], "demonctl-cli");
    assert_eq!(manifest["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(manifest["edges"].as_array().unwrap().len(), 2);

    Ok(())
}

#[test]
fn export_echo_ritual_yaml_output() -> Result<()> {
    let temp = TempDir::new()?;
    let output = temp.path().join("echo_flow.yaml");

    // Get absolute path to project root (parent of demonctl/)
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

    // Verify the file was created
    assert!(output.exists());

    // Verify the file can be parsed as YAML
    let content = fs::read_to_string(&output)?;
    let manifest: serde_yaml::Value = serde_yaml::from_str(&content)?;

    // Verify key fields
    assert_eq!(manifest["schema_version"], "v1");
    assert_eq!(manifest["metadata"]["flow_id"], "flow-echo-ritual");

    Ok(())
}

#[test]
fn export_nonexistent_ritual_fails() {
    Command::cargo_bin("demonctl")
        .unwrap()
        .args([
            "flow",
            "export",
            "--ritual",
            "nonexistent-ritual",
            "--output",
            "/tmp/nonexistent.json",
        ])
        .assert()
        .failure();
}

#[test]
fn import_valid_manifest_dry_run_succeeds() -> Result<()> {
    // Get absolute path to project root (parent of demonctl/)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = std::path::Path::new(manifest_dir).parent().unwrap();
    let manifest_path = project_root.join("examples/flows/hello-agent.json");

    Command::cargo_bin("demonctl")?
        .current_dir(project_root)
        .args([
            "flow",
            "import",
            "--file",
            &manifest_path.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ Manifest validation passed"))
        .stdout(predicate::str::contains("Flow ID: hello-agent-001"))
        .stdout(predicate::str::contains(
            "Dry-run mode: not submitting to API",
        ));

    Ok(())
}

#[test]
fn import_invalid_manifest_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("invalid.json");

    // Create an invalid manifest (missing required fields)
    fs::write(
        &invalid_manifest,
        r#"{"schema_version": "v1", "metadata": {"flow_id": ""}}"#,
    )?;

    Command::cargo_bin("demonctl")?
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn import_manifest_with_invalid_schema_version_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("invalid_version.json");

    // Create a manifest with invalid schema version
    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v99",
            "metadata": {
                "flow_id": "test-001",
                "name": "Test",
                "created_by": "test"
            },
            "nodes": [],
            "edges": []
        }"#,
    )?;

    Command::cargo_bin("demonctl")?
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported schema version: v99"));

    Ok(())
}

#[test]
fn import_manifest_missing_required_fields_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("missing_fields.json");

    // Create a manifest missing required fields
    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v1",
            "metadata": {
                "flow_id": "",
                "name": "Test",
                "created_by": "test"
            },
            "nodes": [],
            "edges": []
        }"#,
    )?;

    Command::cargo_bin("demonctl")?
        .args([
            "flow",
            "import",
            "--file",
            &invalid_manifest.to_string_lossy(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("flow_id is required"));

    Ok(())
}

#[test]
fn import_manifest_duplicate_node_ids_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("duplicate_nodes.json");

    // Create a manifest with duplicate node IDs
    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v1",
            "metadata": {
                "flow_id": "test-001",
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

    Command::cargo_bin("demonctl")?
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
fn import_manifest_invalid_edge_reference_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let invalid_manifest = temp.path().join("invalid_edge.json");

    // Create a manifest with edge referencing non-existent node
    fs::write(
        &invalid_manifest,
        r#"{
            "schema_version": "v1",
            "metadata": {
                "flow_id": "test-001",
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

    Command::cargo_bin("demonctl")?
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
fn help_text_includes_flow_commands() {
    Command::cargo_bin("demonctl")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Flow export/import commands"));
}

#[test]
fn flow_export_help_text() {
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
}

#[test]
fn flow_import_help_text() {
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
