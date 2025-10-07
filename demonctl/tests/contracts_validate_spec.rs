use assert_cmd::Command;
use predicates::str;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[test]
fn given_valid_envelope_file_when_validate_then_exits_successfully() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("valid_envelope.json");

    let envelope = json!({
        "result": {
            "success": true,
            "data": "test"
        }
    });

    fs::write(&file_path, serde_json::to_string(&envelope).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "contracts",
        "validate-envelope",
        file_path.to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(str::contains("✓ Valid envelope"));
}

#[test]
fn given_invalid_envelope_file_when_validate_then_exits_with_error() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("invalid_envelope.json");

    let envelope = json!({
        "invalid_field": "test"
    });

    fs::write(&file_path, serde_json::to_string(&envelope).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "contracts",
        "validate-envelope",
        file_path.to_str().unwrap(),
    ])
    .assert()
    .failure()
    .stderr(str::contains("✗ Invalid envelope"));
}

#[test]
fn given_valid_envelope_on_stdin_when_validate_then_exits_successfully() {
    let envelope = json!({
        "result": {
            "success": true,
            "data": "test"
        }
    });

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["contracts", "validate-envelope", "--stdin"])
        .write_stdin(serde_json::to_string(&envelope).unwrap())
        .assert()
        .success()
        .stdout(str::contains("✓ Valid envelope"));
}

#[test]
fn given_invalid_envelope_on_stdin_when_validate_then_exits_with_error() {
    let envelope = json!({
        "result": {
            "success": true,
            "data": "test"
        },
        "diagnostics": [
            {
                "level": "invalid",
                "message": "test"
            }
        ]
    });

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["contracts", "validate-envelope", "--stdin"])
        .write_stdin(serde_json::to_string(&envelope).unwrap())
        .assert()
        .failure()
        .stderr(str::contains("✗ Invalid envelope"));
}

#[test]
fn given_envelope_with_runtime_and_counts_when_validate_then_exits_successfully() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("metrics_envelope.json");

    let envelope = json!({
        "result": {
            "success": true,
            "data": {"message": "ok"}
        },
        "metrics": {
            "runtime": {
                "capsule": {
                    "name": "hoss-hfab",
                    "duration_ms": 1234
                }
            },
            "counts": {
                "artifacts": {
                    "written": 5,
                    "uploaded": 4
                }
            }
        }
    });

    fs::write(&file_path, serde_json::to_string(&envelope).unwrap()).unwrap();

    Command::cargo_bin("demonctl")
        .unwrap()
        .args([
            "contracts",
            "validate-envelope",
            file_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(str::contains("✓ Valid envelope"));
}

#[test]
fn given_directory_with_result_files_when_bulk_validate_then_processes_all() {
    let temp_dir = TempDir::new().unwrap();

    let valid_envelope = json!({
        "result": {
            "success": true,
            "data": "valid"
        }
    });

    let invalid_envelope = json!({
        "missing_result": true
    });

    fs::write(
        temp_dir.path().join("result.json"),
        serde_json::to_string(&valid_envelope).unwrap(),
    )
    .unwrap();

    let sub_dir = temp_dir.path().join("subdir");
    fs::create_dir(&sub_dir).unwrap();
    fs::write(
        sub_dir.join("result.json"),
        serde_json::to_string(&invalid_envelope).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "contracts",
        "validate-envelope",
        "--bulk",
        temp_dir.path().to_str().unwrap(),
    ])
    .assert()
    .success()
    .stdout(str::contains("Valid: 1"));
}

#[test]
fn given_no_input_specified_when_validate_then_exits_with_error() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["contracts", "validate-envelope"])
        .assert()
        .failure()
        .stderr(str::contains("Must specify"));
}
