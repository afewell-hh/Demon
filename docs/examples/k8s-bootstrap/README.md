# Kubernetes Bootstrap Configuration

This directory contains documentation and examples for the Demon Kubernetes Bootstrap feature.

> 📋 **Release Status**: The Kubernetes bootstrapper is ready for MVP Alpha integration. See [Release Notes (RC1)](../../releases/bootstrapper-rc1.md) for complete feature overview, validation results, and integration guidance.

## Overview

The `demonctl k8s-bootstrap` command provides a streamlined way to deploy Demon on Kubernetes clusters. It handles k3s installation, Demon deployment, secret management, and optional add-ons through a single YAML configuration file.

## Quick Start

1. **Create a configuration file** (see `config.example.yaml` for a complete example):

```yaml
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: my-demon-cluster

cluster:
  name: my-k3s-cluster

demon:
  natsUrl: "nats://nats.demon-system.svc.cluster.local:4222"
  streamName: "RITUAL_EVENTS"
  subjects: ["ritual.>", "approval.>"]
  uiUrl: "http://operate-ui.demon-system.svc.cluster.local:3000"
  namespace: "demon-system"

secrets:
  provider: env
  env:
    GITHUB_TOKEN: GITHUB_TOKEN
    ADMIN_TOKEN: ADMIN_TOKEN
```

2. **Validate your configuration**:
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml --dry-run
```

3. **Deploy to Kubernetes**:
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml
```

## Configuration Reference

### Required Fields

- `apiVersion`: Must be `demon.io/v1`
- `kind`: Must be `BootstrapConfig`
- `metadata.name`: Name for the bootstrap configuration
- `cluster.name`: Name for the Kubernetes cluster
- `demon.natsUrl`: NATS server URL
- `demon.streamName`: NATS JetStream stream name
- `demon.subjects`: List of NATS subjects to subscribe to
- `demon.uiUrl`: Operate UI URL
- `demon.namespace`: Kubernetes namespace for Demon components

### Optional Configuration

#### Cluster Settings
- `cluster.version`: K3s version (default: `v1.28.2+k3s1`)
- `cluster.dataDir`: K3s data directory (default: `/var/lib/rancher/k3s`)
- `cluster.nodeName`: Cluster node name (default: `demon-node`)
- `cluster.extraArgs`: Additional k3s arguments

#### Persistence
- `demon.persistence.enabled`: Enable persistent storage (default: `true`)
- `demon.persistence.storageClass`: Storage class (default: `local-path`)
- `demon.persistence.size`: Storage size (default: `10Gi`)

#### Secret Management
The bootstrapper generates a Kubernetes Secret manifest with your configured secrets, which is applied before other Demon components. The secret is named `demon-secrets` by default.

**Environment Variables Provider** (default):
Maps secret keys to environment variable names. The bootstrapper reads these environment variables and creates a Kubernetes Secret.

```yaml
secrets:
  provider: env
  env:
    # key_in_secret: ENV_VAR_NAME
    github_token: GITHUB_TOKEN     # Reads from $GITHUB_TOKEN
    admin_token: ADMIN_TOKEN       # Reads from $ADMIN_TOKEN
    database_url: DATABASE_URL     # Reads from $DATABASE_URL
```

In dry-run mode, the bootstrapper validates that all environment variables exist without exposing their values.

**HashiCorp Vault Provider**:
Fetches secrets from a Vault server. Currently supports token authentication.

```yaml
secrets:
  provider: vault
  vault:
    address: "https://vault.example.com:8200"  # Optional, defaults to $VAULT_ADDR
    role: "demon-bootstrap"                    # Optional, for Kubernetes auth
    path: "secret/demon/config"                # Path to secrets in Vault
    authMethod: "token"                        # Default: token
```

Required environment variables:
- `VAULT_ADDR` (if not specified in config)
- `VAULT_TOKEN` (for token auth method)

In dry-run mode, the bootstrapper validates the Vault configuration without fetching secrets.

#### Add-ons
The bootstrapper supports an extensible add-on system for optional components like monitoring and observability tools.

```yaml
addons:
  # Monitoring stack (Prometheus + Grafana)
  - name: monitoring
    enabled: true
    config:
      prometheusRetention: "15d"        # Metrics retention period
      prometheusStorageSize: "10Gi"     # Storage size for Prometheus
      grafanaAdminPassword: "admin"     # Grafana admin password
```

**Available Add-ons:**

- **monitoring**: Deploys Prometheus and Grafana for cluster monitoring and observability
  - `prometheusRetention`: How long to retain metrics (default: "15d")
  - `prometheusStorageSize`: Storage size for Prometheus data (default: "10Gi")
  - `grafanaAdminPassword`: Admin password for Grafana (default: "admin")

**Add-on Manifest Generation:**
- Add-ons are processed after core Demon components
- Each enabled add-on generates multiple Kubernetes manifests
- Manifests include services, deployments, configmaps, and RBAC rules
- Add-on manifests respect the configured namespace

**Security Considerations:**
- Default passwords should be changed in production environments
- Add-ons don't expose sensitive configuration in dry-run output
- RBAC rules are scoped to the Demon namespace

#### Networking
```yaml
networking:
  ingress:
    enabled: true
    hostname: demon.local
    tlsSecretName: demon-tls
  serviceMesh:
    enabled: false
```

## CLI Commands

### Bootstrap
Bootstrap a Demon Kubernetes cluster:
```bash
demonctl k8s-bootstrap bootstrap --config <config-file> [--dry-run] [--verbose]
```

**Flags:**
- `--config`: Path to the configuration YAML file (required)
- `--dry-run`: Validate configuration without executing deployment
- `--verbose`: Show detailed configuration and deployment information

### Health Checks

After successful deployment, the bootstrap command automatically verifies that the Demon components are healthy:

**Runtime API Health Check:**
- Verifies the runtime service at `http://localhost:8080/health`
- Executed via `kubectl exec` from within the runtime pod
- Ensures the runtime API is responding correctly

**Operate UI Health Check:**
- Verifies the UI service at `http://localhost:3000/api/runs`
- Executed via `kubectl exec` from within the UI pod
- Ensures the UI is ready to serve requests

**Health Check Failure Handling:**
If health checks fail, the bootstrap command will:
- Exit with a non-zero status code
- Display actionable error messages
- Provide troubleshooting commands for investigation:
  ```bash
  kubectl logs -n <namespace> <pod-name>
  kubectl port-forward -n <namespace> pod/<pod-name> <port>:<port>
  ```

**Note:** Health checks will run after deployment unless using `--dry-run` mode.

### Examples

**Dry run (concise output):**
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml --dry-run
```

Sample output:
```text
✓ Configuration is valid
Dry run mode - no changes will be made
Cluster: my-k3s-cluster (namespace: demon-system)
5 manifests will be generated.
Run with --verbose to view the k3s installation plan and manifest preview.
Note: Health checks will run after deployment to verify runtime API and Operate UI.
```

**Dry run with verbose output (k3s plan + manifest preview):**
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml --dry-run --verbose
```

Sample excerpt:
```text
Configuration summary:
  Cluster: my-k3s-cluster (v1.28.2+k3s1)
  Add-ons: 2
  Secrets: env (3 keys)
    - Keys to be configured: ["github_token", "admin_token", "database_url"]

📋 k3s Installation Plan:
  Version: v1.28.2+k3s1
  Channel: stable
  Data Directory: /var/lib/rancher/k3s

Manifests to be applied:
  - demon-secrets (Secret)
  - namespace.yaml
  - nats.yaml
  - runtime.yaml
  - engine.yaml
  - operate-ui.yaml

Generated manifests:
  (full YAML is printed below this list so you can diff before applying)
```

**Full deployment:**
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml
```

## Deploy Demon

The bootstrap command now supports full Demon deployment to Kubernetes clusters. When not using `--dry-run`, the CLI will:

1. **Install k3s cluster** - Validates configuration and installs k3s with the specified version
2. **Wait for cluster readiness** - Verifies k3s is ready to accept workloads
3. **Generate and apply manifests** - Renders templates with your configuration and applies them via `kubectl`
4. **Wait for pod readiness** - Monitors Demon pods until they reach Ready state (120s timeout)
5. **Run health checks** - Verifies runtime API and Operate UI endpoints are responding correctly

### Manifest Templates

The deployment uses templates located in `demonctl/resources/k8s/`:
- `namespace.yaml` - Creates the Demon namespace
- `nats.yaml` - Deploys NATS server with JetStream
- `runtime.yaml` - Deploys the Demon runtime service
- `engine.yaml` - Deploys the Demon engine
- `operate-ui.yaml` - Deploys the Operate UI

**Add-on Templates** (located in `demonctl/resources/addons/`):
- `monitoring/prometheus-*.yaml` - Prometheus configuration, deployment, and service
- `monitoring/grafana-*.yaml` - Grafana configuration, deployment, and service

Templates support conditional logic for persistence settings:
```yaml
{{- if .persistence.enabled }}
volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      storageClassName: {{ .storageClass }}
{{- else }}
volumes:
  - name: data
    emptyDir: {}
{{- end }}
```

### Verification Commands

After deployment, verify your cluster:
```bash
# Check cluster nodes
sudo k3s kubectl get nodes

# Check Demon pods
sudo k3s kubectl get pods -n your-namespace

# Check services
sudo k3s kubectl get services -n your-namespace

# View logs
sudo k3s kubectl logs -n your-namespace deployment/engine
```

## Implementation Status

### ✅ Completed (Story 1: Core CLI & Configuration)
- [x] CLI subcommand `demonctl k8s-bootstrap bootstrap`
- [x] YAML configuration parsing with serde
- [x] Comprehensive configuration validation
- [x] JSON schema for IDE support (`contracts/schemas/bootstrapper/k8s-config.json`)
- [x] Example configurations with documentation
- [x] Unit tests for config parsing and validation
- [x] CLI integration tests with assert_cmd

### ✅ Completed (Story 2: Demon Deployment)
- [x] Template rendering engine for Kubernetes manifests
- [x] Manifest generation from configuration
- [x] kubectl integration for manifest application
- [x] Pod readiness verification with timeout
- [x] Dry-run manifest preview with --verbose flag
- [x] Command executor abstraction for testable deployment

### ✅ Completed (Story 3: Secret Management)
- [x] Environment variable secret provider
- [x] Vault secret provider integration
- [x] Secret manifest generation with base64 encoding
- [x] Dry-run mode security (no secret exposure)
- [x] Secret manifest applied before other components
- [x] Unit tests for secret collection and rendering
- [x] CLI integration tests for secret scenarios

### ✅ Completed (Story 5: Add-on Plugin System)
- [x] Add-on trait and registry system
- [x] Monitoring add-on with Prometheus and Grafana
- [x] Template-based manifest generation for add-ons
- [x] Configuration validation for add-ons
- [x] Integration with bootstrap flow (dry-run and apply modes)
- [x] Unit tests for add-on framework
- [x] CLI integration tests for add-on scenarios

### ✅ Completed (Story 6: Health Checks & Artifact Capture)
- [x] Runtime API health endpoint verification
- [x] Operate UI health endpoint verification
- [x] Post-deployment health checks with actionable error messages
- [x] Enhanced smoke test artifact collection
- [x] Structured artifact organization with manifests, logs, and descriptions

### 🚧 Future Implementation
- [ ] Rollback capabilities
- [ ] Additional built-in add-ons (logging, service mesh, etc.)

## Validation

The configuration is validated against a JSON schema located at `contracts/schemas/bootstrapper/k8s-config.json`. This enables:

- IDE autocomplete and validation
- Runtime validation with detailed error messages
- Type safety for all configuration options

Common validation errors:
- Missing required fields (`metadata.name`, `cluster.name`, etc.)
- Invalid paths (e.g., relative paths for `cluster.dataDir`)
- Invalid secret provider configurations
- Empty required arrays (e.g., `demon.subjects`)

## Testing

Run the test suite:
```bash
# Unit tests
cargo test -p demonctl k8s_bootstrap

# CLI integration tests
cargo test -p demonctl k8s_bootstrap_cli

# End-to-end smoke test (requires k3d or kind)
make bootstrap-smoke

# Dry-run smoke test only
make bootstrap-smoke ARGS="--dry-run-only"

# Full smoke test with cleanup
make bootstrap-smoke ARGS="--cleanup"
```

## CI Coverage

The bootstrapper has automated CI coverage to catch regressions and ensure quality:

### PR/Push Smoke Testing
- **Job**: `k8s-bootstrapper-smoke-dryrun` in the main CI workflow
- **Trigger**: Runs on PRs/pushes that contain "k8s-bootstrap" in commit messages or titles, or affect related files
- **Scope**: Dry-run validation only (no cluster provisioning)
- **Artifacts**: Configuration validation output and rendered manifests
- **Purpose**: Fast feedback on configuration parsing, template rendering, and validation logic

### Scheduled Full Testing
- **Workflow**: `.github/workflows/bootstrapper-smoke.yml`
- **Schedule**: Daily at 02:00 UTC
- **Scope**: Full end-to-end test with k3d cluster provisioning
- **Artifacts**: Complete cluster state, logs, and health check results
- **Purpose**: Detect integration issues and infrastructure drift

### CI Test Coverage
The automated tests validate:
- ✅ Configuration file parsing and schema validation
- ✅ Template rendering with various configuration options
- ✅ Manifest generation and structure
- ✅ Command-line argument parsing
- ✅ Dry-run mode output formatting
- ✅ Error handling and validation messages
- ✅ (Scheduled only) k3d cluster provisioning and deployment
- ✅ (Scheduled only) Pod readiness verification
- ✅ (Scheduled only) Runtime and UI health checks

### Viewing CI Results
- **Dry-run artifacts**: Available on PR checks as `k8s-bootstrapper-dryrun-artifacts`
- **Full test artifacts**: Available on scheduled runs as `k8s-bootstrapper-smoke-nightly-<run-number>`
- **Summary**: GitHub Actions provides a test summary with status and next scheduled run time

### Smoke Test

The `scripts/tests/smoke-k8s-bootstrap.sh` script provides comprehensive end-to-end testing of the bootstrapper:

**Features:**
- Automatic cluster provisioning (k3d preferred, kind fallback)
- Bootstrap configuration validation and deployment
- Pod readiness verification with timeout
- Service health checks
- Comprehensive logging and artifact capture
- Optional cleanup with `--cleanup` flag

**Usage:**
```bash
# Full smoke test with cluster provisioning
./scripts/tests/smoke-k8s-bootstrap.sh

# Dry-run validation only (no cluster creation)
./scripts/tests/smoke-k8s-bootstrap.sh --dry-run-only

# With cleanup after completion
./scripts/tests/smoke-k8s-bootstrap.sh --cleanup

# With verbose output
./scripts/tests/smoke-k8s-bootstrap.sh --verbose

# Custom configuration
./scripts/tests/smoke-k8s-bootstrap.sh --config ./my-config.yaml
```

**Environment Variables:**
- `DRY_RUN=1` - Same as `--dry-run-only`
- `CLEANUP=1` - Same as `--cleanup`
- `VERBOSE=1` - Same as `--verbose`

**Requirements:**
- k3d (preferred) or kind installed
- kubectl installed
- Docker or Podman runtime
- Built demonctl binary (`cargo build -p demonctl`)

**Artifacts:**
All test artifacts are captured in `dist/bootstrapper-smoke/<timestamp>/` with organized subdirectories:
- `kubeconfig` - Cluster access configuration
- `bootstrap-output.txt` - Bootstrap command output
- `manifests/` - YAML exports of all deployed resources
- `logs/` - Pod logs (current and previous if available)
- `descriptions/` - Detailed Kubernetes resource descriptions
- `runtime-health.txt` & `ui-health.txt` - Health check responses
- `events.txt` - Kubernetes events in the demon namespace
- `final-state.txt` - Final cluster state summary
- `dry-run-output.txt` - Dry-run validation output (dry-run mode only)

The script validates:
✓ Configuration parsing and validation
✓ Template rendering and manifest generation
✓ k3s cluster installation and readiness
✓ Demon pod deployment and readiness
✓ Secret creation and availability
✓ Runtime API health endpoint verification
✓ Operate UI health endpoint verification

## Schema Development

The JSON schema is automatically enforced at runtime. To update the schema:

1. Modify the Rust structs in `demonctl/src/k8s_bootstrap.rs`
2. Update the JSON schema in `contracts/schemas/bootstrapper/k8s-config.json`
3. Run tests to ensure compatibility

## Troubleshooting

### Configuration Validation Errors

**"metadata.name is required"**
- Ensure the `metadata.name` field is not empty

**"cluster.dataDir must be an absolute path"**
- Use absolute paths like `/var/lib/rancher/k3s`, not relative paths

**"demon.subjects cannot be empty"**
- Provide at least one NATS subject pattern

**"vault configuration is required when provider is 'vault'"**
- Include the `vault` section when using `provider: vault`

### CLI Issues

**"Config file is required"**
- Provide the `--config` flag with a valid YAML file path

**"Failed to load config"**
- Check that the config file exists and is valid YAML
- Verify file permissions allow reading

### Health Check Issues

**"Runtime health check failed"**
- Check runtime pod logs: `kubectl logs -n <namespace> deployment/demon-runtime`
- Verify runtime is listening on port 8080: `kubectl port-forward -n <namespace> pod/<runtime-pod> 8080:8080`
- Ensure the runtime has a `/health` endpoint implemented

**"Operate UI health check failed"**
- Check UI pod logs: `kubectl logs -n <namespace> deployment/demon-operate-ui`
- Verify UI is listening on port 3000: `kubectl port-forward -n <namespace> pod/<ui-pod> 3000:3000`
- Ensure the UI has `/api/runs` endpoint available

**"No demon-runtime pod found for health checking"**
- Verify pod deployment: `kubectl get pods -n <namespace> -l app=demon-runtime`
- Check pod status: `kubectl describe pods -n <namespace> -l app=demon-runtime`

For more help, run:
```bash
demonctl k8s-bootstrap --help
```
