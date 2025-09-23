use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn given_help_flag_when_run_k8s_bootstrap_then_shows_usage() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Bootstrap a Kubernetes cluster"))
        .stdout(predicate::str::contains("bootstrap"));
}

#[test]
fn given_no_config_when_run_k8s_bootstrap_then_shows_error() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap").arg("bootstrap");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--config <FILE>"));
}

#[test]
fn given_nonexistent_config_when_run_k8s_bootstrap_then_shows_error() {
    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg("/nonexistent/config.yaml");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn given_invalid_yaml_when_run_k8s_bootstrap_then_shows_error() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "invalid: yaml: [unclosed").unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path());

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("mapping values are not allowed"));
}

#[test]
fn given_invalid_config_when_run_k8s_bootstrap_then_shows_validation_error() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: ""
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path());

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("metadata.name is required"));
}

#[test]
fn given_valid_config_with_dry_run_when_run_k8s_bootstrap_then_validates_only() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: env
  env:
    TEST_VAR: TEST_VALUE
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"))
        .stdout(predicate::str::contains("Dry run mode"))
        .stdout(predicate::str::contains("test-cluster"));
}

#[test]
fn given_valid_config_with_verbose_when_run_k8s_bootstrap_then_shows_detailed_output() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  version: "v1.28.2+k3s1"
  dataDir: "/var/lib/rancher/k3s"
  nodeName: "test-node"
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: env
  env:
    TEST_VAR: TEST_VALUE
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration summary"))
        .stdout(predicate::str::contains("Cluster: test-cluster"))
        .stdout(predicate::str::contains("v1.28.2+k3s1"))
        .stdout(predicate::str::contains("NATS URL: nats://localhost:4222"))
        .stdout(predicate::str::contains("Namespace: test-system"));
}

#[test]
fn given_config_with_vault_secrets_when_run_dry_run_then_validates_vault_config() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: vault
  vault:
    address: "https://vault.example.com"
    role: "test-role"
    path: "secret/test"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration summary"));
}

#[test]
fn given_config_with_addons_when_run_dry_run_then_shows_addon_info() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: env
  env:
    TEST_VAR: TEST_VALUE
addons:
  - name: prometheus
    enabled: true
    values:
      retention: "7d"
  - name: grafana
    enabled: false
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration summary"));
}

#[test]
fn given_config_missing_required_demon_fields_when_validate_then_shows_specific_errors() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: ""
  streamName: ""
  subjects: []
  uiUrl: ""
  namespace: ""
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(
        predicate::str::contains("demon.natsUrl is required").or(predicate::str::contains(
            "demon.streamName is required",
        )
        .or(predicate::str::contains("demon.subjects cannot be empty")
            .or(predicate::str::contains("demon.uiUrl is required")
                .or(predicate::str::contains("demon.namespace is required"))))),
    );
}

#[test]
fn given_config_with_relative_data_dir_when_validate_then_shows_path_error() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  dataDir: "relative/path"
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(predicate::str::contains(
        "cluster.dataDir must be an absolute path",
    ));
}

#[test]
fn given_env_provider_without_env_config_when_validate_then_succeeds() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: env
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

#[test]
fn given_invalid_secrets_provider_when_validate_then_shows_provider_error() {
    let config_yaml = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects: ["test.>"]
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
secrets:
  provider: invalid-provider
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(config_yaml.as_bytes()).unwrap();

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(predicate::str::contains(
        "secrets.provider must be one of: vault, env, file",
    ));
}
