use assert_cmd::prelude::*;
use httptest::responders::status_code;
use httptest::{matchers::request, Expectation, Server};
use predicates::str;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_path(rel: &str) -> PathBuf {
    let candidates = [
        Path::new(rel).to_path_buf(),
        Path::new("..").join(rel),
        Path::new("../..").join(rel),
        Path::new("../../..").join(rel),
    ];
    for p in candidates {
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(rel)
}

fn setup_test_environment(tmp_path: &Path) {
    // Copy the real public key from contracts/keys to the temp directory
    let keys_dir = tmp_path.join("contracts/keys");
    fs::create_dir_all(&keys_dir).unwrap();
    let real_key_path = repo_path("contracts/keys/preview.ed25519.pub");
    let key_content = fs::read_to_string(&real_key_path).unwrap();
    let key_path = keys_dir.join("preview.ed25519.pub");
    fs::write(&key_path, key_content).unwrap();

    // Copy the JSON schema file that the CLI needs for validation
    let schemas_dir = tmp_path.join("contracts/schemas");
    fs::create_dir_all(&schemas_dir).unwrap();
    let real_schema_path = repo_path("contracts/schemas/bootstrap.library.index.v0.json");
    let schema_content = fs::read_to_string(&real_schema_path).unwrap();
    let schema_path = schemas_dir.join("bootstrap.library.index.v0.json");
    fs::write(&schema_path, schema_content).unwrap();
}

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

#[test]
fn bootstrap_https_bundle_verify_only() {
    // Start a test HTTP server
    let server = Server::run();

    // Use the real bundle content from local-dev.yaml
    let bundle_path = repo_path("examples/bundles/local-dev.yaml");
    let bundle_yaml = fs::read_to_string(&bundle_path).unwrap();

    // Set up expectation for bundle download
    server.expect(
        Expectation::matching(request::method_path("GET", "/bundles/local-dev.yaml"))
            .respond_with(status_code(200).body(bundle_yaml)),
    );

    // Create temp directory for index (CLI expects bootstrapper/library/index.json)
    let tmp = tempfile::tempdir().unwrap();
    let idx_dir = tmp.path().join("bootstrapper/library");
    fs::create_dir_all(&idx_dir).unwrap();
    let idx_path = idx_dir.join("index.json");

    // Create index with HTTPS provider using real digest/signature from library index
    let index_content = format!(
        r#"{{
      "provider": "https",
      "baseUrl": "{}",
      "bundles": [{{
        "name": "preview-local-dev",
        "version": "0.0.1",
        "path": "bundles/local-dev.yaml",
        "digest": {{"sha256": "f691d7f0acf56b000bea35321d5dcdfcdc56a0f2f033f49840b86e2438d59445"}},
        "sig": {{"ed25519": "azOENOcSL/BhHwi9TAZQwrCpyR4GYml9kHgJUp9wYrNSoixdog7rF6VJvDYp4JkvO2BJzppRLwDh27Ik38kfCQ"}},
        "pubKeyId": "preview"
      }}]
    }}"#,
        server.url("")
    );
    fs::write(&idx_path, index_content).unwrap();

    // Set up test environment with schema and keys
    setup_test_environment(tmp.path());

    // Run the command with --verify-only
    let mut cmd = Command::cargo_bin("bootstrapper-demonctl").unwrap();

    // Set working directory to temp so it can find the index
    cmd.current_dir(tmp.path());
    cmd.args([
        "--bundle",
        "lib://https/preview-local-dev@0.0.1",
        "--verify-only",
    ])
    .assert()
    .success()
    .stdout(str::contains(r#""phase":"resolve"#))
    .stdout(str::contains(r#""provider":"https"#))
    .stdout(str::contains(r#""name":"preview-local-dev"#));
}

#[ignore]
#[test]
fn bootstrap_https_bundle_http_error() {
    // Start a test HTTP server
    let server = Server::run();

    // Set up expectation for 404 error
    server.expect(
        Expectation::matching(request::method_path("GET", "/bundles/missing.yaml"))
            .respond_with(status_code(404)),
    );

    // Create temp directory for index (CLI expects bootstrapper/library/index.json)
    let tmp = tempfile::tempdir().unwrap();
    let idx_dir = tmp.path().join("bootstrapper/library");
    fs::create_dir_all(&idx_dir).unwrap();
    let idx_path = idx_dir.join("index.json");

    // Create index with HTTPS provider pointing to missing bundle
    let index_content = format!(
        r#"{{
      "provider": "https",
      "baseUrl": "{}",
      "bundles": [{{
        "name": "missing-bundle",
        "version": "1.0.0",
        "path": "bundles/missing.yaml",
        "digest": {{"sha256": "0000000000000000000000000000000000000000000000000000000000000000"}},
        "sig": {{"ed25519": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}},
        "pubKeyId": "preview"
      }}]
    }}"#,
        server.url("")
    );
    fs::write(&idx_path, index_content).unwrap();

    // Set up test environment with schema and keys
    setup_test_environment(tmp.path());

    // Run the command with --verify-only
    let mut cmd = Command::cargo_bin("bootstrapper-demonctl").unwrap();

    // Set working directory to temp so it can find the index
    cmd.current_dir(tmp.path());
    cmd.args([
        "--bundle",
        "lib://https/missing-bundle@1.0.0",
        "--verify-only",
    ])
    .assert()
    .failure()
    .stderr(str::contains("HTTP error 404"));
}
