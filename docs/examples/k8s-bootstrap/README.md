# Kubernetes Bootstrap Configuration

This directory contains documentation and examples for the Demon Kubernetes Bootstrap feature.

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
```yaml
addons:
  - name: prometheus
    enabled: true
    values:
      retention: "7d"
      storage: "5Gi"
```

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

### Manifest Templates

The deployment uses templates located in `demonctl/resources/k8s/`:
- `namespace.yaml` - Creates the Demon namespace
- `nats.yaml` - Deploys NATS server with JetStream
- `runtime.yaml` - Deploys the Demon runtime service
- `engine.yaml` - Deploys the Demon engine
- `operate-ui.yaml` - Deploys the Operate UI

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

### 🚧 Future Implementation
- [ ] Add-on plugin system
- [ ] Health checks and verification
- [ ] Rollback capabilities

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
```

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

For more help, run:
```bash
demonctl k8s-bootstrap --help
```
