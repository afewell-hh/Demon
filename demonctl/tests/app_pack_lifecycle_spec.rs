use anyhow::Result;
use assert_cmd::Command;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

// ==================== INSTALL TESTS ====================

#[test]
fn given_valid_unsigned_pack_when_install_then_succeeds() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "test-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    let installed_manifest = install_home.join("packs/test-app/1.0.0/app-pack.yaml");
    assert!(
        installed_manifest.exists(),
        "Manifest should be installed at {}",
        installed_manifest.display()
    );

    let installed_contract = install_home.join("packs/test-app/1.0.0/contracts/test/contract.json");
    assert!(
        installed_contract.exists(),
        "Contract should be installed at {}",
        installed_contract.display()
    );

    Ok(())
}

#[test]
fn given_pack_already_installed_when_install_same_version_then_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "duplicate-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // First install should succeed
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    // Second install without --overwrite should fail
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn given_pack_already_installed_when_install_with_overwrite_then_succeeds() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "overwrite-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // First install
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    // Second install with --overwrite should succeed
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", "--overwrite", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    Ok(())
}

#[test]
fn given_pack_with_missing_contract_when_install_then_fails() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    fs::create_dir_all(&pack_dir)?;

    // Create manifest that references a contract that doesn't exist
    let manifest_content = r#"
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: missing-contract-app
  version: 1.0.0
contracts:
  - id: missing/contract
    version: 1.0.0
    path: contracts/missing.json
capsules:
  - type: container-exec
    name: noop
    imageDigest: ghcr.io/example/noop@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    command: ["/bin/true"]
    outputs:
      envelopePath: /workspace/.artifacts/result.json
rituals:
  - name: noop
    steps:
      - capsule: noop
"#;
    fs::write(pack_dir.join("app-pack.yaml"), manifest_content)?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .failure();

    Ok(())
}

#[test]
fn given_invalid_manifest_when_install_then_fails_with_validation_error() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    fs::create_dir_all(&pack_dir)?;

    // Create manifest with missing required fields
    let manifest_content = r#"
apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: invalid-app
  # missing version field - should fail validation
"#;
    fs::write(pack_dir.join("app-pack.yaml"), manifest_content)?;

    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .failure();

    Ok(())
}

// ==================== UNINSTALL TESTS ====================

#[test]
fn given_installed_pack_when_uninstall_then_removes_files_and_registry() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "uninstall-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // Install first
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    let installed_path = install_home.join("packs/uninstall-app/1.0.0");
    assert!(installed_path.exists(), "Pack should be installed");

    // Uninstall
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "uninstall", "uninstall-app"])
        .assert()
        .success();

    assert!(
        !installed_path.exists(),
        "Pack directory should be removed after uninstall"
    );

    // Verify registry is empty
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list", "--json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));

    Ok(())
}

#[test]
fn given_installed_pack_when_uninstall_with_retain_files_then_removes_registry_only() -> Result<()>
{
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "retain-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // Install
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    let installed_path = install_home.join("packs/retain-app/1.0.0");

    // Uninstall with --retain-files
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "uninstall", "--retain-files", "retain-app"])
        .assert()
        .success();

    assert!(
        installed_path.exists(),
        "Pack directory should be retained when --retain-files is used"
    );

    // Verify registry is empty
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list", "--json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));

    Ok(())
}

#[test]
fn given_multiple_versions_when_uninstall_specific_version_then_removes_only_that_version(
) -> Result<()> {
    let temp = TempDir::new()?;
    let install_home = temp.path().join("home");

    // Install version 1.0.0
    let pack_dir_v1 = temp.path().join("pack-v1");
    create_minimal_unsigned_pack(&pack_dir_v1, "multi-version-app", "1.0.0")?;
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir_v1.to_string_lossy()])
        .assert()
        .success();

    // Install version 2.0.0
    let pack_dir_v2 = temp.path().join("pack-v2");
    create_minimal_unsigned_pack(&pack_dir_v2, "multi-version-app", "2.0.0")?;
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir_v2.to_string_lossy()])
        .assert()
        .success();

    // Uninstall only version 1.0.0
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args([
            "app",
            "uninstall",
            "multi-version-app",
            "--version",
            "1.0.0",
        ])
        .assert()
        .success();

    let v1_path = install_home.join("packs/multi-version-app/1.0.0");
    let v2_path = install_home.join("packs/multi-version-app/2.0.0");

    assert!(!v1_path.exists(), "Version 1.0.0 should be removed");
    assert!(v2_path.exists(), "Version 2.0.0 should still exist");

    Ok(())
}

#[test]
fn given_nonexistent_pack_when_uninstall_then_fails_gracefully() -> Result<()> {
    let temp = TempDir::new()?;
    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "uninstall", "nonexistent-app"])
        .assert()
        .failure();

    Ok(())
}

// ==================== LIST TESTS ====================

#[test]
fn given_no_packs_installed_when_list_then_shows_empty_message() -> Result<()> {
    let temp = TempDir::new()?;
    let install_home = temp.path().join("home");

    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No App Packs installed"));

    Ok(())
}

#[test]
fn given_installed_packs_when_list_then_shows_pack_details() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "list-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // Install a pack
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    // List should show the pack
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("list-app"))
        .stdout(predicates::str::contains("1.0.0"));

    Ok(())
}

#[test]
fn given_installed_packs_when_list_with_json_then_outputs_valid_json() -> Result<()> {
    let temp = TempDir::new()?;
    let pack_dir = temp.path().join("pack");
    create_minimal_unsigned_pack(&pack_dir, "json-app", "1.0.0")?;

    let install_home = temp.path().join("home");

    // Install a pack
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir.to_string_lossy()])
        .assert()
        .success();

    // List with --json should output valid JSON
    let output = Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list", "--json"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone())?;
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;

    assert!(parsed.is_array(), "JSON output should be an array");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1, "Should have one installed pack");
    assert_eq!(
        arr[0]["name"].as_str().unwrap(),
        "json-app",
        "Pack name should match"
    );
    assert_eq!(
        arr[0]["version"].as_str().unwrap(),
        "1.0.0",
        "Pack version should match"
    );

    Ok(())
}

#[test]
fn given_multiple_packs_when_list_then_shows_all_packs() -> Result<()> {
    let temp = TempDir::new()?;
    let install_home = temp.path().join("home");

    // Install first pack
    let pack_dir_1 = temp.path().join("pack1");
    create_minimal_unsigned_pack(&pack_dir_1, "app-one", "1.0.0")?;
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir_1.to_string_lossy()])
        .assert()
        .success();

    // Install second pack
    let pack_dir_2 = temp.path().join("pack2");
    create_minimal_unsigned_pack(&pack_dir_2, "app-two", "2.0.0")?;
    Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "install", &pack_dir_2.to_string_lossy()])
        .assert()
        .success();

    // List should show both packs
    let output = Command::cargo_bin("demonctl")?
        .env("DEMON_APP_HOME", &install_home)
        .args(["app", "list", "--json"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone())?;
    let parsed: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(
        parsed.as_array().unwrap().len(),
        2,
        "Should have two installed packs"
    );

    Ok(())
}

// ==================== HELPER FUNCTIONS ====================

fn create_minimal_unsigned_pack(root: &std::path::Path, name: &str, version: &str) -> Result<()> {
    fs::create_dir_all(root.join("contracts/test"))?;

    // Write a minimal contract
    let contract_path = root.join("contracts/test/contract.json");
    fs::write(contract_path, r#"{"type":"object"}"#)?;

    // Build manifest
    let manifest = json!({
        "apiVersion": "demon.io/v1",
        "kind": "AppPack",
        "metadata": {
            "name": name,
            "version": version,
            "displayName": format!("{} Display Name", name),
            "description": format!("Test app pack for {}", name),
        },
        "contracts": [
            {
                "id": format!("{}/contract", name),
                "version": version,
                "path": "contracts/test/contract.json"
            }
        ],
        "capsules": [
            {
                "type": "container-exec",
                "name": "noop",
                "imageDigest": "ghcr.io/example/noop@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "command": ["/bin/true"],
                "outputs": {
                    "envelopePath": "/workspace/.artifacts/result.json"
                }
            }
        ],
        "rituals": [
            {
                "name": "noop",
                "displayName": "No-op Ritual",
                "description": "A ritual that does nothing",
                "steps": [
                    { "capsule": "noop" }
                ]
            }
        ]
    });

    let yaml = serde_yaml::to_string(&manifest).expect("failed to serialize manifest to YAML");
    let manifest_path = root.join("app-pack.yaml");
    fs::write(&manifest_path, yaml.trim_start_matches("---\n"))?;

    Ok(())
}
