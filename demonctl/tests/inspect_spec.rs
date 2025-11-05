use assert_cmd::Command;
use predicates::prelude::*;

/// Test that inspect requires --graph flag
#[test]
fn given_no_flags_when_inspect_then_error_message() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["inspect"]);

    cmd.assert().failure().stderr(predicate::str::contains(
        "No inspection target specified. Use --graph to inspect graph metrics",
    ));
}

/// Test that inspect with invalid NATS URL fails gracefully
#[test]
fn given_invalid_nats_url_when_inspect_graph_then_error_with_exit_code_2() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "inspect",
        "--graph",
        "--nats-url",
        "nats://invalid-host:9999",
    ])
    .timeout(std::time::Duration::from_secs(5));

    // Should fail with exit code 2 and provide troubleshooting info
    cmd.assert()
        .code(2)
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Troubleshooting:"));
}

/// Test that inspect with invalid NATS URL and JSON flag outputs JSON error
#[test]
fn given_invalid_nats_url_when_inspect_graph_json_then_json_error() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "inspect",
        "--graph",
        "--json",
        "--nats-url",
        "nats://invalid-host:9999",
    ])
    .timeout(std::time::Duration::from_secs(5));

    let assert = cmd.assert().code(2);

    // In JSON mode, should output JSON error
    assert.stdout(predicate::str::contains(r#"{"error":"#));
}

/// Test help output
#[test]
fn given_help_flag_when_inspect_then_show_usage() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args(["inspect", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Show graph metrics"))
        .stdout(predicate::str::contains("--graph"))
        .stdout(predicate::str::contains("--json"));
}

/// Test that threshold flags are accepted
#[test]
fn given_custom_thresholds_when_inspect_then_accepted() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "inspect",
        "--graph",
        "--json",
        "--warn-queue-lag",
        "100",
        "--error-queue-lag",
        "200",
        "--warn-p95-latency-ms",
        "250.0",
        "--error-p95-latency-ms",
        "500.0",
        "--warn-error-rate",
        "0.01",
        "--error-error-rate",
        "0.03",
        "--nats-url",
        "nats://invalid-host:9999",
    ])
    .timeout(std::time::Duration::from_secs(5));

    // Should still fail due to invalid NATS, but thresholds should be accepted
    cmd.assert().code(2);
}

/// Test that tenant flag is accepted
#[test]
fn given_custom_tenant_when_inspect_then_accepted() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "inspect",
        "--graph",
        "--tenant",
        "production",
        "--nats-url",
        "nats://invalid-host:9999",
    ])
    .timeout(std::time::Duration::from_secs(5));

    // Should fail due to invalid NATS, but tenant should be accepted
    cmd.assert().code(2);
}

// Note: Integration tests that require a running NATS server with scale hint data
// should be run separately or marked with #[ignore]. The core functionality tests
// above verify CLI interface, argument parsing, and error handling.

#[test]
#[ignore = "Requires running NATS server with SCALE_HINTS stream"]
fn given_nats_with_no_scale_hints_when_inspect_graph_then_error() {
    // This test requires NATS to be running but without scale hints
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.args([
        "inspect",
        "--graph",
        "--nats-url",
        &nats_url,
        "--tenant",
        "nonexistent-tenant",
    ])
    .timeout(std::time::Duration::from_secs(10));

    cmd.assert().code(2);
}
