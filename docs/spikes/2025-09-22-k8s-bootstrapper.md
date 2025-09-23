# Kubernetes Bootstrapper Spike — Issue #139

**Date**: 2025-09-22
**Author**: AI Assistant (Claude Code)
**Status**: Design Complete, Ready for Implementation
**Related**: Issue #139 - Feature Request - Demon Kubernetes bootstrapper

## Executive Summary

This spike provides a complete design for a Kubernetes bootstrapper that deploys single-node k3s clusters with Demon fully installed and configured. The solution leverages existing `bootstrapper-demonctl` infrastructure while adding k3s orchestration, configuration templating, and extensibility for future add-on applications.

### Key Outcomes
- **Target**: Single-node k3s deployment with full Demon stack
- **Architecture**: Extend existing `bootstrapper-demonctl` with k8s orchestration layer
- **Configuration**: YAML-driven with secret injection and templating
- **Extensibility**: Plugin architecture for future k8s applications
- **Security**: Vault integration, signed bundles, secure token handling

## Requirements Analysis

### From Issue #139
- ✅ **Core Requirement**: Deploy k3s + Demon automatically with configuration input
- ✅ **Distribution**: Single-node k3s (as specified)
- ✅ **Configuration**: Accept input parameters/config file for Demon configuration
- ✅ **Extensibility**: Built to support future k8s application add-ons
- ✅ **Integration**: Support applications needing tight k8s integration beyond WASM

### Derived Requirements
- **Idempotency**: Multiple runs should be safe and convergent
- **Verification**: Health checks and readiness validation
- **Rollback**: Ability to uninstall/reset cluster state
- **Observability**: Logging and progress tracking throughout bootstrap
- **Security**: Secrets management, signed artifacts, secure defaults

## Current State Analysis

### Existing Assets (Reusable)
1. **`bootstrapper-demonctl`** (`bootstrapper/demonctl/`)
   - ✅ Configuration merging and validation
   - ✅ NATS stream management (`ensure_stream`)
   - ✅ Bundle loading and provenance verification
   - ✅ UI verification and health checks
   - ✅ Signed bundle resolution (`lib://` URIs)

2. **Docker Compose Dev Environment** (`docker/dev/`)
   - ✅ NATS JetStream orchestration patterns
   - ✅ Health check implementations
   - ✅ Port management and service discovery

3. **Contract Release Infrastructure**
   - ✅ Bundle signing and verification
   - ✅ SHA-256 integrity checks
   - ✅ Metadata management and versioning

### Gaps to Address
- ❌ **K8s orchestration**: No existing k3s installation/management
- ❌ **K8s-specific config**: Need Demon → k8s deployment translation
- ❌ **Cluster lifecycle**: Install, upgrade, uninstall workflows
- ❌ **Add-on framework**: Plugin system for additional k8s apps

## Proposed Architecture

### Component Overview

```
k8s-bootstrapper/
├── cli/                    # New CLI interface
│   ├── src/
│   │   ├── main.rs        # CLI entry point
│   │   ├── commands/      # Bootstrap, status, destroy commands
│   │   └── config.rs      # Configuration schema
│   └── Cargo.toml
├── core/                   # Core orchestration logic
│   ├── src/
│   │   ├── k3s.rs         # K3s installation and management
│   │   ├── demon.rs       # Demon deployment to k8s
│   │   ├── addons.rs      # Add-on plugin system
│   │   ├── templates.rs   # Configuration templating
│   │   └── verification.rs # Health checks and validation
│   └── Cargo.toml
├── templates/              # Kubernetes manifests
│   ├── demon/             # Core Demon k8s resources
│   │   ├── namespace.yaml
│   │   ├── nats.yaml      # NATS JetStream deployment
│   │   ├── engine.yaml    # Demon engine deployment
│   │   ├── runtime.yaml   # Demon runtime deployment
│   │   ├── operate-ui.yaml # UI deployment
│   │   └── ingress.yaml   # Optional ingress config
│   └── addons/            # Extension point for add-ons
│       └── examples/
├── config/                 # Configuration schemas and examples
│   ├── schema.json        # JSON schema for validation
│   ├── examples/
│   │   ├── minimal.yaml   # Basic Demon deployment
│   │   ├── vault.yaml     # With Vault secrets integration
│   │   └── full.yaml      # All options documented
│   └── defaults.yaml      # Sensible defaults
└── tests/                  # Integration tests
    ├── e2e/               # End-to-end scenarios
    └── fixtures/          # Test configurations
```

### Integration with Existing Infrastructure

**Reuse `bootstrapper-demonctl` as Foundation**:
- Extend `BootstrapConfig` with k8s-specific fields
- Leverage existing bundle loading, signing, verification
- Reuse NATS configuration and stream management
- Maintain compatibility with current CLI interface

**New `k8s-bootstrapper` CLI**:
- Wraps `bootstrapper-demonctl` for Demon-specific logic
- Adds k3s installation, k8s deployment, cluster lifecycle
- Provides unified interface: `demon-k8s bootstrap config.yaml`

## Detailed Design

### 1. Configuration Schema

```yaml
# config.yaml - User input
apiVersion: demon.io/v1
kind: BootstrapConfig
metadata:
  name: my-demon-cluster

# K3s cluster configuration
cluster:
  name: demon-cluster
  version: "v1.28.2+k3s1"
  dataDir: /var/lib/rancher/k3s
  nodeName: demon-node
  # Optional: custom k3s install options
  extraArgs:
    - "--disable=traefik"  # Disable default ingress
    - "--disable=servicelb"

# Demon configuration (extends existing BootstrapConfig)
demon:
  # Reuse existing demonctl config structure
  natsUrl: "nats://nats.demon-system.svc.cluster.local:4222"
  streamName: "RITUAL_EVENTS"
  subjects: ["ritual.>", "approval.>"]
  dedupeWindowSecs: 60
  uiUrl: "http://operate-ui.demon-system.svc.cluster.local:3000"

  # K8s-specific overrides
  namespace: demon-system
  persistence:
    enabled: true
    storageClass: local-path  # k3s default
    size: 10Gi

  # Optional bundle for advanced config
  bundle: "lib://demon-stack/v1.0.0"

# Secret management
secrets:
  provider: vault  # vault | env | file
  vault:
    address: "https://vault.example.com"
    role: "demon-bootstrap"
    path: "secret/demon"
  # Alternatively: file paths or env var names
  env:
    GITHUB_TOKEN: GITHUB_TOKEN
    ADMIN_TOKEN: ADMIN_TOKEN

# Optional add-ons (extensibility)
addons:
  - name: prometheus
    enabled: true
    values:
      retention: 7d
  - name: grafana
    enabled: false

# Networking
networking:
  ingress:
    enabled: true
    hostname: demon.local
    tlsSecretName: demon-tls
  serviceMesh:
    enabled: false
```

### 2. Bootstrap Flow

```rust
// Simplified flow in core/src/lib.rs
pub async fn bootstrap(config: &BootstrapConfig) -> Result<()> {
    // Phase 1: Pre-flight checks
    preflight::verify_system_requirements()?;
    preflight::validate_config(config)?;

    // Phase 2: K3s cluster setup
    let cluster = k3s::install_k3s(&config.cluster).await?;
    k3s::wait_for_cluster_ready(&cluster, Duration::from_secs(300)).await?;

    // Phase 3: Kubernetes namespace and RBAC
    let k8s_client = kubernetes::create_client(&cluster).await?;
    kubernetes::create_namespace(&k8s_client, &config.demon.namespace).await?;
    kubernetes::apply_rbac(&k8s_client, &config).await?;

    // Phase 4: Secret injection
    secrets::inject_secrets(&k8s_client, &config.secrets).await?;

    // Phase 5: Deploy Demon stack
    demon::deploy_nats(&k8s_client, &config).await?;
    demon::deploy_engine(&k8s_client, &config).await?;
    demon::deploy_runtime(&k8s_client, &config).await?;
    demon::deploy_operate_ui(&k8s_client, &config).await?;

    // Phase 6: Configure networking
    networking::setup_ingress(&k8s_client, &config).await?;

    // Phase 7: Bootstrap Demon (reuse existing demonctl)
    let demon_config = config.to_demon_bootstrap_config();
    bootstrapper_demonctl::run_all(&demon_config, "bootstrap", None).await?;

    // Phase 8: Deploy add-ons
    for addon in &config.addons {
        addons::deploy_addon(&k8s_client, addon).await?;
    }

    // Phase 9: Final verification
    verification::verify_demon_health(&demon_config).await?;
    verification::verify_k8s_resources(&k8s_client, &config).await?;

    println!("✅ Demon Kubernetes cluster ready!");
    Ok(())
}
```

### 3. CLI Interface

```bash
# Install k3s + Demon with config file
demon-k8s bootstrap config.yaml

# Status and health checks
demon-k8s status
demon-k8s health

# Individual component management
demon-k8s demon --restart
demon-k8s nats --scale 3

# Add-on management
demon-k8s addon install prometheus
demon-k8s addon list
demon-k8s addon remove grafana

# Cluster lifecycle
demon-k8s upgrade --k3s-version v1.29.0+k3s1
demon-k8s destroy  # Full cluster teardown

# Debug and troubleshooting
demon-k8s logs --component engine
demon-k8s debug --export-config
```

### 4. Kubernetes Resource Templates

**NATS JetStream Deployment** (`templates/demon/nats.yaml`):
```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: nats
  namespace: {{ .namespace }}
spec:
  serviceName: nats
  replicas: 1
  selector:
    matchLabels:
      app: nats
  template:
    metadata:
      labels:
        app: nats
    spec:
      containers:
      - name: nats
        image: nats:2.10
        args: ["-js", "-sd", "/data"]
        ports:
        - containerPort: 4222
          name: client
        - containerPort: 8222
          name: monitor
        volumeMounts:
        - name: data
          mountPath: /data
        livenessProbe:
          httpGet:
            path: /varz
            port: 8222
          initialDelaySeconds: 10
          periodSeconds: 10
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: {{ .persistence.storageClass }}
      resources:
        requests:
          storage: {{ .persistence.size }}
```

### 5. Secret Management Integration

**Vault Provider** (`core/src/secrets/vault.rs`):
```rust
pub async fn inject_vault_secrets(
    client: &Client,
    config: &VaultConfig,
    namespace: &str,
) -> Result<()> {
    let vault_client = VaultClient::new(&config.address).await?;
    let auth_token = vault_client.auth_kubernetes(&config.role).await?;

    let secrets = vault_client.read_secrets(&config.path, &auth_token).await?;

    for (key, value) in secrets {
        let secret = Secret {
            metadata: ObjectMeta {
                name: Some(key.clone()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some({
                let mut data = BTreeMap::new();
                data.insert(key, ByteString::from(value.as_bytes().to_vec()));
                data
            }),
            ..Default::default()
        };

        client.create(&PostParams::default(), &secret).await?;
    }

    Ok(())
}
```

## Work Breakdown & Stories

### Epic: K8s Bootstrapper Foundation (MVP-Beta)
**Estimated Effort**: 3-4 sprints
**Dependencies**: Contract Registry (#121) complete

#### Story 1: Core CLI and Configuration (8 points)
- [ ] Create `k8s-bootstrapper` workspace crate
- [ ] Design and implement configuration schema with JSON validation
- [ ] Build CLI with clap for `bootstrap`, `status`, `destroy` commands
- [ ] Add configuration validation and error handling
- [ ] Create example configurations (minimal, vault, full)

**Acceptance Criteria**:
- CLI can parse and validate bootstrap configuration files
- `demon-k8s bootstrap --dry-run config.yaml` validates without errors
- Configuration schema supports all documented options
- Help text and examples are clear and comprehensive

#### Story 2: K3s Installation and Management (13 points)
- [ ] Implement k3s download and installation automation
- [ ] Add cluster health checks and readiness verification
- [ ] Create kubeconfig management and client setup
- [ ] Implement cluster lifecycle (install, uninstall, upgrade)
- [ ] Add pre-flight system requirement checks

**Acceptance Criteria**:
- Can install k3s cluster from scratch on clean system
- Cluster readiness verification works reliably
- Failed installations can be cleanly removed
- Supports different k3s versions and configuration options

#### Story 3: Demon Stack K8s Deployment (21 points)
- [ ] Create Kubernetes manifest templates for Demon components
- [ ] Implement template rendering with configuration injection
- [ ] Port existing `bootstrapper-demonctl` to work with k8s deployments
- [ ] Add persistence and storage management
- [ ] Implement health checks for all Demon components

**Acceptance Criteria**:
- All Demon components deploy successfully to k8s
- NATS persistence works across pod restarts
- Demon engine and runtime can communicate via k8s services
- UI is accessible and shows healthy cluster status
- Existing Demon bundles work without modification

#### Story 4: Secret Management Integration (8 points)
- [ ] Implement Vault secrets provider
- [ ] Add environment variable and file-based secret sources
- [ ] Create secure secret injection into k8s
- [ ] Add secret rotation and management
- [ ] Document security best practices

**Acceptance Criteria**:
- Vault integration retrieves and injects secrets successfully
- Secrets are properly encrypted at rest in k8s
- Multiple secret backends work (vault, env, file)
- No secrets are logged or exposed in plain text

#### Story 5: Add-on Plugin System (13 points)
- [ ] Design plugin architecture for k8s add-ons
- [ ] Implement example add-ons (Prometheus, Grafana)
- [ ] Create add-on lifecycle management (install, upgrade, remove)
- [ ] Add add-on dependency resolution
- [ ] Document add-on development guidelines

**Acceptance Criteria**:
- Can install and remove add-ons without affecting Demon
- Add-ons have access to Demon metrics and logs
- Plugin system is documented for future extensions
- Example add-ons work out of the box

### Epic: Production Readiness (Future)
**Estimated Effort**: 2-3 sprints
**Dependencies**: MVP-Beta complete

#### Story 6: High Availability and Scaling (21 points)
- [ ] Multi-node k3s cluster support
- [ ] NATS clustering and replication
- [ ] Load balancing and ingress configuration
- [ ] Backup and disaster recovery procedures

#### Story 7: Monitoring and Observability (13 points)
- [ ] Integrated monitoring stack deployment
- [ ] Demon-specific dashboards and alerts
- [ ] Log aggregation and analysis
- [ ] Performance monitoring and optimization

#### Story 8: Security Hardening (8 points)
- [ ] Network policies and segmentation
- [ ] RBAC fine-tuning and least privilege
- [ ] Security scanning and vulnerability management
- [ ] Compliance reporting and auditing

## Open Questions & Risks

### Technical Questions for @afewell-hh

1. **Cloud Environment Support**: Should we support cloud providers (AWS EKS, GCP GKE) in addition to k3s, or is single-node k3s sufficient for initial release?

2. **Persistent Storage**: What are the requirements for Demon data persistence? Should we support distributed storage backends or is local storage adequate?

3. **Networking Requirements**: Do you have specific requirements for service mesh, load balancing, or ingress controllers?

4. **Resource Limits**: What are the expected resource requirements (CPU, memory, storage) for a typical Demon deployment?

5. **Update Strategy**: How should we handle updates to Demon components? Rolling updates, blue-green deployments, or manual coordination?

### Implementation Risks

- **Medium Risk**: K3s installation complexity across different Linux distributions
  - *Mitigation*: Start with Ubuntu/Debian support, expand incrementally

- **Low Risk**: Secret management complexity with multiple providers
  - *Mitigation*: Start with environment variables, add Vault integration later

- **Medium Risk**: Kubernetes resource conflicts with existing deployments
  - *Mitigation*: Use dedicated namespaces and clear naming conventions

- **High Risk**: Integration testing complexity for full stack
  - *Mitigation*: Invest in CI/CD pipeline with automated testing infrastructure

## Next Steps

### Immediate Actions (Sprint Planning)
1. **Create Epic and Stories**: Add to GitHub project with MVP-Beta milestone
2. **Architecture Review**: Schedule design review with @afewell-hh
3. **Proof of Concept**: Build minimal k3s + Demon bootstrap in ~1 week
4. **CI/CD Planning**: Design test infrastructure for multi-component validation

### MVP Delivery Timeline
- **Week 1-2**: Stories 1-2 (CLI + K3s)
- **Week 3-5**: Story 3 (Demon deployment)
- **Week 6-7**: Story 4 (Secrets)
- **Week 8-9**: Story 5 (Add-ons) + Integration testing
- **Week 10**: Documentation, polish, release prep

### Definition of Done
- [ ] Complete bootstrap workflow works end-to-end
- [ ] Documentation covers installation, configuration, troubleshooting
- [ ] CI pipeline validates on Ubuntu 20.04+ and 22.04+
- [ ] Performance benchmarks establish baseline requirements
- [ ] Security review completed with no high-severity findings

---

**Approval**: Ready for stakeholder review and sprint planning.
**Contact**: Generated by Claude Code AI Assistant for @afewell-hh review.