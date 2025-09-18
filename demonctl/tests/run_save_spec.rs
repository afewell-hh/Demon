use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn workspace_root() -> String {
    // From demonctl/tests, go up to workspace root
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .display()
        .to_string()
}

#[test]
fn run_without_save_works() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let root = workspace_root();
    cmd.current_dir(&root)
        .args(["run", "examples/rituals/echo.yaml"])
        .assert()
        .success();
}

#[test]
fn run_with_save_creates_result_json() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    let root = workspace_root();
    cmd.current_dir(&root)
        .args([
            "run",
            "examples/rituals/echo.yaml",
            "--save",
            "--output-dir",
            temp_dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Check that result.json was created
    let result_path = temp_dir.path().join("result.json");
    assert!(result_path.exists(), "result.json should be created");

    // Check that the file contains valid JSON
    let content = fs::read_to_string(&result_path).unwrap();
    let _: serde_json::Value =
        serde_json::from_str(&content).expect("result.json should contain valid JSON");
}

#[test]
fn run_with_save_to_current_dir() {
    let temp_dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    // Get the workspace root before changing directories
    let root = workspace_root();

    // Run from temp_dir as current directory
    cmd.current_dir(&temp_dir)
        .args([
            "run",
            &format!("{}/examples/rituals/echo.yaml", root),
            "--save",
        ])
        .assert()
        .success();

    // Check that result.json was created in temp_dir (our working directory when we ran the command)
    let result_path = temp_dir.path().join("result.json");
    assert!(
        result_path.exists(),
        "result.json should be created in current directory"
    );
}

#[test]
fn run_with_save_validates_envelope_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    let root = workspace_root();
    cmd.current_dir(&root)
        .args([
            "run",
            "examples/rituals/echo.yaml",
            "--save",
            "--output-dir",
            temp_dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    // Read and validate the envelope structure
    let result_path = temp_dir.path().join("result.json");
    let content = fs::read_to_string(&result_path).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Check for required envelope fields
    assert!(
        envelope.get("result").is_some(),
        "envelope should have result field"
    );

    // The result should have a success field and data field
    let result = envelope.get("result").unwrap();
    assert!(
        result.get("success").is_some(),
        "result should have success field"
    );
    assert!(
        result.get("data").is_some(),
        "result should have data field"
    );

    // Should have provenance, diagnostics, and metrics
    assert!(
        envelope.get("provenance").is_some(),
        "envelope should have provenance field"
    );
    assert!(
        envelope.get("diagnostics").is_some(),
        "envelope should have diagnostics field"
    );
    assert!(
        envelope.get("metrics").is_some(),
        "envelope should have metrics field"
    );
}

#[test]
fn run_help_shows_save_options() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let output = cmd.args(["run", "--help"]).output().unwrap();
    let help_text = String::from_utf8_lossy(&output.stdout);

    assert!(
        help_text.contains("--save"),
        "help should mention --save flag"
    );
    assert!(
        help_text.contains("--output-dir"),
        "help should mention --output-dir flag"
    );
}
