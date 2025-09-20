use assert_cmd::Command;
use predicates::str;
use std::path::Path;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).parent().unwrap().to_path_buf()
}

#[test]
fn test_contracts_bundle_summary_format() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    cmd.current_dir(workspace_root())
        .args(["contracts", "bundle"])
        .assert()
        .success()
        .stdout(str::contains("Contract Bundle Summary"))
        .stdout(str::contains("Schemas"))
        .stdout(str::contains("result-envelope.json"));
}

#[test]
fn test_contracts_bundle_with_wit() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    cmd.current_dir(workspace_root())
        .args(["contracts", "bundle", "--include-wit"])
        .assert()
        .success()
        .stdout(str::contains("Contract Bundle Summary"))
        .stdout(str::contains("WIT Definitions"))
        .stdout(str::contains("demon-envelope.wit"));
}

#[test]
fn test_contracts_bundle_json_format() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    let output = cmd
        .current_dir(workspace_root())
        .args(["contracts", "bundle", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Output should be valid JSON");

    assert!(
        json.get("version").is_some(),
        "JSON should have version field"
    );
    assert!(
        json.get("schemas").is_some(),
        "JSON should have schemas field"
    );

    // Check that result envelope schema is included
    let schemas = json.get("schemas").unwrap().as_object().unwrap();
    assert!(
        schemas.contains_key("result-envelope.json"),
        "Schemas should include result-envelope.json"
    );
}

#[test]
fn test_contracts_bundle_json_with_wit() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    let output = cmd
        .current_dir(workspace_root())
        .args(["contracts", "bundle", "--format", "json", "--include-wit"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Output should be valid JSON");

    assert!(
        json.get("wit").is_some(),
        "JSON should have wit field when --include-wit is used"
    );

    let wit_defs = json.get("wit").unwrap().as_object().unwrap();
    assert!(
        wit_defs.contains_key("demon-envelope.wit"),
        "WIT definitions should include demon-envelope.wit"
    );
}

#[test]
fn test_contracts_bundle_includes_all_schemas() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    let output = cmd
        .current_dir(workspace_root())
        .args(["contracts", "bundle", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&json_str).expect("Output should be valid JSON");

    let schemas = json.get("schemas").unwrap().as_object().unwrap();

    // Check for key schema files
    assert!(
        schemas.contains_key("result-envelope.json"),
        "Should include result envelope"
    );
    assert!(
        schemas.contains_key("bootstrap.bundle.v0.json"),
        "Should include bootstrap bundle"
    );
    assert!(
        schemas.contains_key("events.ritual.started.v1.json"),
        "Should include ritual events"
    );
}
