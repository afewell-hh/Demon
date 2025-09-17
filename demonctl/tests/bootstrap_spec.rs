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
