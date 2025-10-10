#![cfg(unix)]

use anyhow::Result;
use assert_cmd::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn workspace_root() -> std::path::PathBuf {
    Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

#[test]
fn demonctl_run_uses_installed_workspace_mount_for_scripts() -> Result<()> {
    // Gate on docker availability
    let docker_available = Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !docker_available {
        return Ok(());
    }

    // Ensure alpine image is present and get digest used across tests
    let _ = Command::new("docker")
        .args(["pull", "alpine:3.20"])
        .status();
    let alpine_digest = "docker.io/library/alpine@sha256:b3119ef930faabb6b7b976780c0c7a9c1aa24d0c75e9179ac10e6bc9ac080d0d";

    let temp = TempDir::new()?;
    let app_home = temp.path().join("app-home");
    fs::create_dir_all(&app_home)?;

    // Build a minimal app-pack with a capsule script under capsules/
    let pack_dir = temp.path().join("pack");
    fs::create_dir_all(pack_dir.join("contracts/test"))?;
    fs::create_dir_all(pack_dir.join("capsules/test/scripts"))?;
    fs::write(pack_dir.join("contracts/test/result.json"), b"{}")?;

    let script_path = pack_dir.join("capsules/test/scripts/hello.sh");
    fs::write(
        &script_path,
        b"#!/bin/sh\nset -eu\numask 077\nprintf '{\"result\":{\"success\":true,\"data\":{\"note\":\"script-ok\"}},\"diagnostics\":[]}' > \"$ENVELOPE_PATH\"\n",
    )?;
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;

    let manifest = format!(
        r#"apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: ws
  version: 0.1.0
contracts:
  - id: test/contracts/result
    version: 0.1.0
    path: contracts/test/result.json
capsules:
  - type: container-exec
    name: runner
    imageDigest: {digest}
    command:
      - /bin/sh
      - -c
      - "/workspace/capsules/test/scripts/hello.sh"
    outputs:
      envelopePath: /workspace/.artifacts/summary.json
rituals:
  - name: run
    steps:
      - capsule: runner
"#,
        digest = alpine_digest
    );

    fs::write(pack_dir.join("app-pack.yaml"), manifest)?;

    let workspace = workspace_root();

    // Install the app-pack (should copy entire tree, including capsules/)
    Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .args(["app", "install", pack_dir.to_str().unwrap(), "--overwrite"])
        .assert()
        .success();

    // Run via alias and expect success; script must be found under /workspace/capsules
    let output = Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .env("DEMON_CONTAINER_USER", "1000:1000")
        .env("DEMON_DEBUG", "1")
        .args(["run", "ws:run"])
        .output()?;

    assert!(output.status.success(), "demonctl run failed: {:?}", output);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["outputs"]["result"]["success"], true);
    Ok(())
}
