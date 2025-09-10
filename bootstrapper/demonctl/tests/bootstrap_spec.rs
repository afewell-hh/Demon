use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn arg_parse_help() {
    let mut cmd = Command::cargo_bin("bootstrapper-demonctl").unwrap();
    cmd.arg("--help").assert().success();
}

#[ignore]
#[test]
fn bootstrap_is_idempotent() {
    let mut cmd = Command::cargo_bin("bootstrapper-demonctl").unwrap();
    // run ensure + seed + verify; requires NATS and UI running as per CI
    cmd.args([
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--verify",
    ])
    .assert()
    .success();

    let mut cmd2 = Command::cargo_bin("bootstrapper-demonctl").unwrap();
    cmd2.args([
        "--profile",
        "local-dev",
        "--ensure-stream",
        "--seed",
        "--verify",
    ])
    .assert()
    .success();
}
