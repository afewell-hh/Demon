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

3. **Deploy to Kubernetes** (future implementation):
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
Supports three providers:

**Environment Variables** (default):
```yaml
secrets:
  provider: env
  env:
    VAR_NAME: ENV_VAR_NAME
```

**HashiCorp Vault**:
```yaml
secrets:
  provider: vault
  vault:
    address: "https://vault.example.com"
    role: "demon-bootstrap"
    path: "secret/demon"
```

**File Provider**:
```yaml
secrets:
  provider: file
```

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

**Dry run with verbose output:**
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml --dry-run --verbose
```

**Full deployment:**
```bash
demonctl k8s-bootstrap bootstrap --config config.yaml
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

### 🚧 Future Implementation
- [ ] K3s installation and management
- [ ] Kubernetes manifest deployment
- [ ] Secret injection (Vault, env, file)
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