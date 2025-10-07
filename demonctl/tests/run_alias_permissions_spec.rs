#![cfg(unix)]

use anyhow::Result;
use assert_cmd::prelude::*;
use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

#[test]
fn run_alias_persists_envelope_with_custom_runtime() -> Result<()> {
    let temp = TempDir::new()?;
    let app_home = temp.path().join("app-home");
    fs::create_dir_all(&app_home)?;

    let pack_dir = temp.path().join("pack");
    fs::create_dir_all(pack_dir.join("contracts/hoss"))?;
    fs::write(pack_dir.join("contracts/hoss/result.json"), b"{}")?;
    fs::write(
        pack_dir.join("app-pack.yaml"),
        r#"apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: hoss
  version: 0.1.0
contracts:
  - id: hoss/contracts/result
    version: 0.1.0
    path: contracts/hoss/result.json
capsules:
  - type: container-exec
    name: validator
    imageDigest: ghcr.io/example/validator@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    command:
      - /bin/sh
      - -c
      - "exit 0"
    outputs:
      envelopePath: /workspace/.artifacts/summary.json
rituals:
  - name: hoss-validate
    steps:
      - capsule: validator
"#,
    )?;

    let runtime_bin = temp.path().join("fake-docker.sh");
    fs::write(
        &runtime_bin,
        r##"#!/usr/bin/env bash
set -euo pipefail

host=""
envelope=""
args=("$@")
for ((i=0; i<${#args[@]}; i++)); do
  arg="${args[$i]}"
  if [[ "$arg" == "--mount" ]]; then
    i=$((i+1))
    mount="${args[$i]}"
    if [[ "$mount" == type=bind,source=* ]] && [[ "$mount" == *,target=/workspace/.artifacts* ]]; then
      host="${mount#type=bind,source=}"
      host="${host%%,target=*}"
    fi
  elif [[ "$arg" == "--env" ]]; then
    i=$((i+1))
    envspec="${args[$i]}"
    if [[ "$envspec" == ENVELOPE_PATH=* ]]; then
      envelope="${envspec#ENVELOPE_PATH=}"
    fi
  fi
done

if [[ -z "$host" || -z "$envelope" ]]; then
  echo "missing mount or envelope" >&2
  exit 1
fi

rel="${envelope#/workspace/.artifacts/}"
if [[ "$rel" == "$envelope" ]]; then
  rel="${envelope#/}"
fi

mkdir -p "$(dirname "$host/$rel")"
cat > "$host/$rel" <<'JSON'
{"result":{"success":true,"data":{"source":"fake-runtime"}}}
JSON

exit 0
"##,
    )?;
    fs::set_permissions(&runtime_bin, fs::Permissions::from_mode(0o755))?;

    let workspace = workspace_root();

    Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .args(["app", "install", pack_dir.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .env("DEMON_CONTAINER_RUNTIME", runtime_bin.as_os_str())
        .args(["run", "hoss:hoss-validate"])
        .assert()
        .success();

    Ok(())
}

#[test]
fn run_alias_emits_envelope_with_real_runtime() -> Result<()> {
    if Command::new("docker")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
        == false
    {
        return Ok(());
    }

    let temp = TempDir::new()?;
    let app_home = temp.path().join("app-home");
    fs::create_dir_all(&app_home)?;

    let pack_dir = temp.path().join("pack");
    fs::create_dir_all(pack_dir.join("contracts/test"))?;
    fs::write(pack_dir.join("contracts/test/result.json"), b"{}")?;

    let status = Command::new("docker")
        .args(["pull", "alpine:3.20"])
        .status()?;
    if !status.success() {
        anyhow::bail!("docker pull alpine:3.20 failed");
    }

    let alpine_digest = "docker.io/library/alpine@sha256:b3119ef930faabb6b7b976780c0c7a9c1aa24d0c75e9179ac10e6bc9ac080d0d";

    let manifest = format!(
        r#"apiVersion: demon.io/v1
kind: AppPack
metadata:
  name: hoss
  version: 0.1.0
contracts:
  - id: hoss/contracts/result
    version: 0.1.0
    path: contracts/test/result.json
capsules:
  - type: container-exec
    name: validator
    imageDigest: {digest}
    command:
      - /bin/sh
      - -c
      - "umask 077 && printf '{{\"result\":{{\"success\":true,\"data\":{{}}}},\"diagnostics\":[]}}' > \"$ENVELOPE_PATH\""
    outputs:
      envelopePath: /workspace/.artifacts/summary.json
rituals:
  - name: hoss-validate
    steps:
      - capsule: validator
"#,
        digest = alpine_digest
    );

    fs::write(pack_dir.join("app-pack.yaml"), manifest)?;

    let workspace = workspace_root();

    Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .args([
            "app",
            "install",
            pack_dir.join("app-pack.yaml").to_str().unwrap(),
        ])
        .assert()
        .success();

    let output = Command::cargo_bin("demonctl")?
        .current_dir(&workspace)
        .env("DEMON_APP_HOME", &app_home)
        .args(["run", "hoss:hoss-validate"])
        .output()?;

    assert!(output.status.success(), "demonctl run failed: {:?}", output);

    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        value["outputs"]["result"]["success"].as_bool(),
        Some(true),
        "alias run should succeed: {}",
        value
    );

    Ok(())
}
