use assert_cmd::Command;
use predicates::str;
use std::path::Path;
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).parent().unwrap().to_path_buf()
}

#[test]
fn test_contracts_fetch_bundle_help() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    cmd.current_dir(workspace_root())
        .args(["contracts", "fetch-bundle", "--help"])
        .assert()
        .success()
        .stdout(str::contains("Fetch contract bundle from GitHub releases"))
        .stdout(str::contains("--tag"))
        .stdout(str::contains("--dest"));
}

#[test]
fn test_contracts_fetch_bundle_dry_run() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let temp_dir = TempDir::new().unwrap();

    let result = cmd
        .current_dir(workspace_root())
        .args([
            "contracts",
            "fetch-bundle",
            "--dry-run",
            "--dest",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Check that command starts properly and shows expected messages
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Fetching contract bundle"));
    assert!(stdout.contains("Dry run mode"));

    // Ensure no files were created in dry-run mode
    assert!(temp_dir.path().read_dir().unwrap().next().is_none());
}

#[test]
fn test_contracts_fetch_bundle_dry_run_json_format() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let temp_dir = TempDir::new().unwrap();

    let output = cmd
        .current_dir(workspace_root())
        .args([
            "contracts",
            "fetch-bundle",
            "--dry-run",
            "--format",
            "json",
            "--dest",
            temp_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let json_str = String::from_utf8_lossy(&output.stdout);
    // Check that it starts with JSON-like output
    // The command might fail if no releases exist, but should still output initial JSON
    assert!(json_str.contains("\"phase\": \"fetch\""));
    assert!(json_str.contains("\"dry_run\": true"));
}

#[test]
fn test_contracts_list_releases_help() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    cmd.current_dir(workspace_root())
        .args(["contracts", "list-releases", "--help"])
        .assert()
        .success()
        .stdout(str::contains("List available contract bundle releases"))
        .stdout(str::contains("--limit"))
        .stdout(str::contains("--format"));
}

#[test]
fn test_contracts_list_releases_json_format() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();

    // This test might fail if no GitHub token is available or network is down
    // We'll check if it runs without panicking and produces valid JSON-like output
    let result = cmd
        .current_dir(workspace_root())
        .args([
            "contracts",
            "list-releases",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .output();

    match result {
        Ok(output) => {
            // If successful, check that it's either valid JSON or an error message
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Either it succeeds with JSON output or warns about no token
            if output.status.success() && !stdout.is_empty() {
                // Try to parse as JSON array (might be empty)
                let json_result: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&stdout);
                assert!(
                    json_result.is_ok() || stdout.contains("[]"),
                    "Output should be valid JSON"
                );
            } else {
                // It's okay if it fails due to network or auth issues
                assert!(
                    stderr.contains("GitHub token") || stderr.contains("Failed to fetch"),
                    "Should have meaningful error message"
                );
            }
        }
        Err(_) => {
            // Network error is acceptable in tests
            eprintln!("Network error during test - this is acceptable in CI");
        }
    }
}

#[test]
fn test_contracts_fetch_bundle_invalid_tag() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let temp_dir = TempDir::new().unwrap();

    // This should fail with a non-existent tag
    let result = cmd
        .current_dir(workspace_root())
        .args([
            "contracts",
            "fetch-bundle",
            "--tag",
            "non-existent-tag-99999",
            "--dest",
            temp_dir.path().to_str().unwrap(),
        ])
        .output();

    match result {
        Ok(output) => {
            // Should fail with meaningful error
            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("not found") || stderr.contains("Failed to fetch"),
                "Should have meaningful error for invalid tag"
            );
        }
        Err(_) => {
            // Network error is acceptable
            eprintln!("Network error during test - this is acceptable");
        }
    }
}

#[test]
fn test_contracts_fetch_bundle_custom_destination() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let custom_dest = temp_dir.path().join("my-contracts");

    let result = cmd
        .current_dir(workspace_root())
        .args([
            "contracts",
            "fetch-bundle",
            "--dry-run",
            "--dest",
            custom_dest.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&result.stdout);
    // Check that the custom destination is mentioned
    assert!(stdout.contains(custom_dest.to_str().unwrap()));
}
