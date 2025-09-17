use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn bootstrap_help() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["bootstrap", "--help"]).assert().success();
}

#[test]
fn bootstrap_version() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("version").assert().success();
}

#[ignore]
#[test]
fn bootstrap_ensure_stream_only() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    // run ensure-stream only; requires NATS running as per CI
    cmd.args(["bootstrap", "--profile", "local-dev", "--ensure-stream"])
        .assert()
        .success();
}

#[ignore]
#[test]
fn bootstrap_all_steps() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    // run ensure + seed + verify; requires NATS and UI running as per CI
    cmd.args([
        "bootstrap",
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--verify",
    ])
    .assert()
    .success();
}

#[ignore]
#[test]
fn bootstrap_is_idempotent() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    // run ensure + seed + verify; requires NATS and UI running as per CI
    cmd.args([
        "bootstrap",
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--verify",
    ])
    .assert()
    .success();

    let mut cmd2 = Command::cargo_bin("demonctl").unwrap();
    cmd2.args([
        "bootstrap",
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--verify",
    ])
    .assert()
    .success();
}

#[ignore]
#[test]
fn bootstrap_custom_ritual_id() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "bootstrap",
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--ritual-id",
        "test-ritual",
    ])
    .assert()
    .success();
}

#[ignore]
#[test]
fn bootstrap_with_overrides() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "bootstrap",
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--nats-url",
        "nats://127.0.0.1:4222",
        "--stream-name",
        "TEST_STREAM",
        "--ui-base-url",
        "http://127.0.0.1:3000",
    ])
    .assert()
    .success();
}

#[test]
fn bootstrap_profile_to_bundle_mapping_local_dev() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    // Use a non-existent operation to get config output without side effects
    let output = cmd
        .args(["bootstrap", "--profile", "local-dev", "--verify-only"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should contain effective config based on local-dev bundle defaults in output
    assert!(combined.contains("\"phase\":\"config\""));
    // Should show that it used the profile default bundle in the provenance
    assert!(combined.contains("\"provenance\""));
    // Verify it fails appropriately for verify-only without lib:// URI
    assert!(combined.contains("--verify-only requires --bundle lib://local/... URI"));
}

#[test]
fn bootstrap_profile_to_bundle_mapping_remote_nats() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let output = cmd
        .args(["bootstrap", "--profile", "remote-nats", "--verify-only"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should contain effective config based on remote-nats bundle defaults in output
    assert!(combined.contains("\"phase\":\"config\""));
    // Should show that it used the profile default bundle in the provenance
    assert!(combined.contains("\"provenance\""));
    // Verify it fails appropriately for verify-only without lib:// URI
    assert!(combined.contains("--verify-only requires --bundle lib://local/... URI"));
}

#[test]
fn bootstrap_explicit_bundle_overrides_profile() {
    // Find the remote-nats bundle using the path resolution logic
    let bundle_path = bootstrapper_demonctl::get_default_bundle_for_profile(
        &bootstrapper_demonctl::Profile::RemoteNats,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    let output = cmd
        .args([
            "bootstrap",
            "--profile",
            "local-dev",
            "--bundle",
            &bundle_path,
            "--verify-only",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should contain config from the explicitly provided bundle, not the profile default
    assert!(combined.contains("\"phase\":\"config\""));
    // Should also fail appropriately for verify-only without lib:// URI
    assert!(combined.contains("--verify-only requires --bundle lib://local/... URI"));
}
