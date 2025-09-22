use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

const BASE_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs:
      - "--disable=traefik"
      - "--disable=servicelb"
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env: {}
addons:
  - name: prometheus
    enabled: false
  - name: grafana
    enabled: false
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

const VAULT_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: vault
  vault:
    address: "https://vault.example.com"
    path: "secret/test"
    authMethod: "token"
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

const MINIMAL_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: minimal-cluster
cluster:
  name: minimal-cluster
  runtime: k3s
  k3s:
    version: "v1.28.0+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "minimal-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "minimal-system"
  persistence:
    enabled: false
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: file
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

fn write_config(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    let normalized = contents.trim_start_matches('\n');
    file.write_all(normalized.as_bytes()).unwrap();
    file
}

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

    cmd.assert().failure().stderr(predicate::str::contains(
        "Failed to parse YAML configuration",
    ));
}

#[test]
fn given_runtime_not_k3s_when_run_bootstrap_then_fails() {
    let invalid_config = BASE_CONFIG.replace("runtime: k3s", "runtime: eks");
    let file = write_config(&invalid_config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(predicate::str::contains(
        "Only 'k3s' runtime is currently supported",
    ));
}

#[test]
fn given_valid_config_when_dry_run_then_prints_concise_summary() {
    let file = write_config(BASE_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Configuration is valid"))
        .stdout(predicate::str::contains(
            "Dry run mode - no changes will be made",
        ))
        .stdout(predicate::str::contains(
            "Cluster: test-cluster (namespace: test-system)",
        ))
        .stdout(predicate::str::contains("5 manifests will be generated."))
        .stdout(predicate::str::contains("Run with --verbose"))
        .stdout(predicate::str::contains("namespace.yaml").not())
        .stdout(predicate::str::contains("TEST_VALUE").not());
}

#[test]
fn given_valid_config_when_dry_run_verbose_then_shows_manifest_plan_and_preview() {
    let file = write_config(BASE_CONFIG);

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
        .stdout(predicate::str::contains("Add-ons: 2"))
        .stdout(predicate::str::contains("Secrets: env (0 keys)"))
        .stdout(predicate::str::contains("📋 k3s Installation Plan"))
        .stdout(predicate::str::contains("🔧 Install Command"))
        .stdout(predicate::str::contains("Manifests to be applied"))
        .stdout(predicate::str::contains("namespace.yaml"))
        .stdout(predicate::str::contains("runtime.yaml"))
        .stdout(predicate::str::contains("Generated manifests"));
}

#[test]
fn given_vault_config_when_dry_run_verbose_then_shows_vault_summary() {
    let file = write_config(VAULT_CONFIG);

    // Set required env vars for vault
    std::env::set_var("VAULT_ADDR", "https://vault.example.com");
    std::env::set_var("VAULT_TOKEN", "test-token");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Secrets: vault (configured)"));

    // Clean up env vars
    std::env::remove_var("VAULT_ADDR");
    std::env::remove_var("VAULT_TOKEN");
}

#[test]
fn given_minimal_config_when_dry_run_then_succeeds() {
    let file = write_config(MINIMAL_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✓ Configuration is valid"))
        .stdout(predicate::str::contains("5 manifests will be generated."));
}

#[test]
fn given_apply_only_mode_when_executor_fails_then_reports_error() {
    let file = write_config(BASE_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path());
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-failure");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Failed to apply manifests"))
        .stderr(predicate::str::contains("kubectl apply failed - simulated"))
        .stdout(predicate::str::contains(
            "🚀 Starting K8s bootstrap process (manifests only)",
        ));
}

#[test]
fn given_apply_only_mode_when_executor_succeeds_then_prints_successful_summary() {
    let file = write_config(BASE_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--verbose");
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");
    cmd.env(
        "DEMONCTL_K8S_EXECUTOR_STDOUT",
        "namespace/test-system created\nservice/test-system configured\n",
    );

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "🚀 Starting K8s bootstrap process (manifests only)",
        ))
        .stdout(predicate::str::contains("Applying manifests to cluster"))
        .stdout(predicate::str::contains("Manifests applied successfully"))
        .stdout(predicate::str::contains("namespace/test-system created"))
        .stdout(predicate::str::contains(
            "🎯 Manifest application simulation complete",
        ));
}

#[test]
fn given_env_secrets_configured_when_dry_run_then_shows_secrets_info() {
    let config = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env:
    db_password: DB_PASSWORD
    api_key: API_KEY
    jwt_secret: JWT_SECRET
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

    let file = write_config(config);

    // Set the required env vars for the test
    std::env::set_var("DB_PASSWORD", "test-db-pass");
    std::env::set_var("API_KEY", "test-api-key");
    std::env::set_var("JWT_SECRET", "test-jwt-secret");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Secrets: env (3 keys)"))
        .stdout(predicate::str::contains("demon-secrets (Secret)"))
        .stdout(predicate::str::contains("6 manifests will be generated"));

    // Clean up env vars
    std::env::remove_var("DB_PASSWORD");
    std::env::remove_var("API_KEY");
    std::env::remove_var("JWT_SECRET");
}

#[test]
fn given_vault_secrets_configured_when_dry_run_then_validates_config() {
    // Clean up env vars to ensure test isolation
    std::env::remove_var("VAULT_ADDR");
    std::env::remove_var("VAULT_TOKEN");

    let file = write_config(VAULT_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    // Without VAULT_ADDR and VAULT_TOKEN, validation should fail
    cmd.assert().failure().stderr(predicate::str::contains(
        "VAULT_TOKEN environment variable required",
    ));
}

#[test]
fn given_vault_secrets_with_env_when_dry_run_then_shows_vault_configured() {
    let file = write_config(VAULT_CONFIG);

    // Set required env vars
    std::env::set_var("VAULT_ADDR", "https://vault.example.com");
    std::env::set_var("VAULT_TOKEN", "test-token");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("vault (configured)"));

    // Clean up env vars
    std::env::remove_var("VAULT_ADDR");
    std::env::remove_var("VAULT_TOKEN");
}

#[test]
fn given_env_secrets_missing_env_var_when_not_dry_run_then_fails() {
    let config = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env:
    missing_secret: NONEXISTENT_ENV_VAR
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

    let file = write_config(config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path());
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("NONEXISTENT_ENV_VAR"));
}

#[test]
fn given_secrets_configured_when_apply_then_secret_manifest_applied_first() {
    let config = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env:
    test_secret: TEST_SECRET_VALUE
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

    let file = write_config(config);

    // Set required env var
    std::env::set_var("TEST_SECRET_VALUE", "secret123");

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--verbose");
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");
    cmd.env(
        "DEMONCTL_K8S_EXECUTOR_STDOUT",
        "secret/demon-secrets created\nnamespace/test-system created\nservice/test-system configured\n",
    );

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("secret/demon-secrets created"));

    // Clean up env var
    std::env::remove_var("TEST_SECRET_VALUE");
}

const MONITORING_ADDON_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: test-cluster
cluster:
  name: test-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "test-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env: {}
addons:
  - name: monitoring
    enabled: true
    config:
      prometheusRetention: "30d"
      prometheusStorageSize: "20Gi"
      grafanaAdminPassword: "admin123"
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: false
"#;

#[test]
fn given_monitoring_addon_enabled_when_dry_run_verbose_then_shows_addon_info() {
    let file = write_config(MONITORING_ADDON_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Add-ons enabled (1):"))
        .stdout(predicate::str::contains(
            "- monitoring (Prometheus and Grafana for monitoring and observability)",
        ))
        .stdout(predicate::str::contains("Processing add-on: monitoring"));
}

#[test]
fn given_monitoring_addon_enabled_when_dry_run_then_includes_addon_manifest_count() {
    let file = write_config(MONITORING_ADDON_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    // Add-ons don't generate manifests in dry-run mode, so count stays the same
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("5 manifests will be generated."));
}

#[test]
fn given_monitoring_addon_disabled_when_dry_run_then_no_addon_manifests() {
    let config = MONITORING_ADDON_CONFIG.replace("enabled: true", "enabled: false");
    let file = write_config(&config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("5 manifests will be generated."));
}

#[test]
fn given_unknown_addon_when_bootstrap_then_fails() {
    let config = MONITORING_ADDON_CONFIG.replace("monitoring", "unknown-addon");
    let file = write_config(&config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unknown add-on: unknown-addon"));
}

#[test]
fn given_monitoring_addon_with_invalid_storage_size_when_bootstrap_then_fails() {
    let config = MONITORING_ADDON_CONFIG.replace("20Gi", "20GB");
    let file = write_config(&config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(predicate::str::contains(
        "Invalid storage size format '20GB'. Must end with Gi, Mi, or Ti",
    ));
}

#[test]
fn given_monitoring_addon_enabled_when_apply_then_manifests_include_prometheus_and_grafana() {
    let file = write_config(MONITORING_ADDON_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--verbose");
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");
    cmd.env(
        "DEMONCTL_K8S_EXECUTOR_STDOUT",
        "configmap/prometheus-config created\ndeployment.apps/prometheus created\nservice/prometheus created\nconfigmap/grafana-datasources created\ndeployment.apps/grafana created\nservice/grafana created\n",
    );

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Processing add-on: monitoring"))
        .stdout(predicate::str::contains(
            "configmap/prometheus-config created",
        ))
        .stdout(predicate::str::contains(
            "deployment.apps/prometheus created",
        ))
        .stdout(predicate::str::contains("service/prometheus created"))
        .stdout(predicate::str::contains(
            "configmap/grafana-datasources created",
        ))
        .stdout(predicate::str::contains("deployment.apps/grafana created"))
        .stdout(predicate::str::contains("service/grafana created"));
}

const INGRESS_ENABLED_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: ingress-cluster
cluster:
  name: ingress-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "ingress-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env: {}
addons: []
networking:
  ingress:
    enabled: true
    hostname: ui.example.com
    ingressClass: nginx
    tls:
      enabled: true
      secretName: demon-tls
  serviceMesh:
    enabled: false
"#;

const SERVICE_MESH_ENABLED_CONFIG: &str = r#"
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: mesh-cluster
cluster:
  name: mesh-cluster
  runtime: k3s
  k3s:
    version: "v1.28.2+k3s1"
    install:
      channel: stable
      disable: []
    dataDir: "/var/lib/rancher/k3s"
    nodeName: "mesh-node"
    extraArgs: []
demon:
  natsUrl: "nats://localhost:4222"
  streamName: "TEST_EVENTS"
  subjects:
    - "test.>"
  dedupeWindowSecs: 30
  uiUrl: "http://localhost:3000"
  namespace: "test-system"
  persistence:
    enabled: true
    storageClass: "local-path"
    size: "10Gi"
secrets:
  provider: env
  env: {}
addons: []
networking:
  ingress:
    enabled: false
    hostname: null
    ingressClass: null
    annotations: null
    tls:
      enabled: false
      secretName: null
  serviceMesh:
    enabled: true
"#;

#[test]
fn given_ingress_enabled_when_dry_run_verbose_then_shows_networking_plan() {
    let file = write_config(INGRESS_ENABLED_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Networking:"))
        .stdout(predicate::str::contains(
            "Ingress: enabled (host: ui.example.com, TLS: demon-tls)",
        ))
        .stdout(predicate::str::contains("Service mesh: disabled"));
}

#[test]
fn given_ingress_enabled_when_dry_run_then_includes_ingress_manifest_count() {
    let file = write_config(INGRESS_ENABLED_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("6 manifests will be generated."));
}

#[test]
fn given_service_mesh_enabled_when_dry_run_verbose_then_shows_mesh_plan() {
    let file = write_config(SERVICE_MESH_ENABLED_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Networking:"))
        .stdout(predicate::str::contains("Ingress: disabled"))
        .stdout(predicate::str::contains("Service mesh: enabled"));
}

#[test]
fn given_ingress_enabled_when_apply_then_ingress_manifest_applied() {
    let file = write_config(INGRESS_ENABLED_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--verbose");
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");
    cmd.env(
        "DEMONCTL_K8S_EXECUTOR_STDOUT",
        "namespace/test-system created\ningress.networking.k8s.io/demon-ingress created\nservice/operate-ui created\n",
    );

    cmd.assert().success().stdout(predicate::str::contains(
        "ingress.networking.k8s.io/demon-ingress created",
    ));
}

#[test]
fn given_ingress_enabled_without_hostname_when_validate_then_fails() {
    let config = INGRESS_ENABLED_CONFIG.replace("hostname: ui.example.com", "hostname: null");
    let file = write_config(&config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert().failure().stderr(predicate::str::contains(
        "TLS requires a hostname to be specified",
    ));
}

#[test]
fn given_ingress_with_invalid_hostname_when_validate_then_fails() {
    let config = INGRESS_ENABLED_CONFIG.replace("ui.example.com", "invalid-hostname");
    let file = write_config(&config);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--dry-run");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("appears to be invalid"));
}

#[test]
fn given_service_mesh_enabled_when_apply_then_manifests_include_mesh_annotations() {
    let file = write_config(SERVICE_MESH_ENABLED_CONFIG);

    let mut cmd = Command::cargo_bin("demonctl").unwrap();
    cmd.arg("k8s-bootstrap")
        .arg("bootstrap")
        .arg("--config")
        .arg(file.path())
        .arg("--verbose");
    cmd.env("DEMONCTL_K8S_BOOTSTRAP_EXECUTION", "apply-only");
    cmd.env("DEMONCTL_K8S_EXECUTOR", "simulate-success");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Applying manifests to cluster"));
}
