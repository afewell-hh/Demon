# Platform Engineers & Operators Guide

Welcome, platform engineers! This guide helps you deploy, configure, and operate Demon in production environments.

## 🚀 Quick Start

Ready to deploy Demon? Follow this path:

1. **[Bootstrap Setup](../bootstrapper/README.md)** - Zero-config deployment
2. **[Production Configuration](#production-deployment)** - Security and scaling
3. **[Monitoring Setup](#-monitoring-and-observability)** - Operational visibility

```bash
# Quick bootstrap
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify
```

## 🏗️ Deployment

### Zero-Config Bootstrap
The simplest way to get Demon running:

```bash
# Complete bootstrap with verification
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify

# Individual steps for control
cargo run -p demonctl -- bootstrap --ensure-stream    # Create NATS stream
cargo run -p demonctl -- bootstrap --seed            # Seed sample events
cargo run -p demonctl -- bootstrap --verify          # Verify UI health
```

### Production Deployment
- [Bundle Library & Signatures](../bootstrapper/bundles.md) - Secure package deployment
- [Self-Host Bootstrap Guide](../../README.md#self-host-bootstrap) - Comprehensive setup
- [NATS JetStream Configuration](../../docker/dev/) - Message persistence setup

### Environment Configuration
```bash
# Customize stream and monitoring
RITUAL_STREAM_NAME=CUSTOM_STREAM cargo run -p demonctl -- bootstrap --ensure-stream
NATS_PORT=4222 NATS_MON_PORT=8222 make dev

# Production settings
export WARDS_CAP_QUOTAS="limit:100,windowSeconds:3600"
export APPROVAL_TTL_SECONDS=1800
```

## ⚙️ Configuration Management

### Core Settings
- **NATS Configuration** - Message streaming and persistence
- **Approval TTL** - Automatic denial timeouts
- **Policy Quotas** - Rate limiting and resource controls
- **Stream Names** - Multi-tenant isolation

### Security Configuration
- [Policy and Approvals](../adr/ADR-0003-wards-policy-and-approvals.md) - Security model
- [Wards Per-Cap Quotas](../adr/ADR-0004-wards-per-cap-quotas.md) - Resource limits
- [Bundle Provenance](../adr/ADR-0007-bundle-library-and-provenance.md) - Package security

### Environment Variables
| Variable | Purpose | Default | Example |
|----------|---------|---------|---------|
| `NATS_PORT` | NATS server port | 4222 | 4222 |
| `NATS_MON_PORT` | NATS monitoring port | 8222 | 8222 |
| `RITUAL_STREAM_NAME` | JetStream name | RITUAL_EVENTS | CUSTOM_STREAM |
| `APPROVAL_TTL_SECONDS` | Auto-deny timeout | 3600 | 1800 |
| `WARDS_CAP_QUOTAS` | Rate limit config | - | "limit:100,windowSeconds:3600" |

## 📊 Monitoring and Observability

### Health Checks
```bash
# Verify system health
cargo run -p demonctl -- bootstrap --verify

# Check NATS stream status
nats stream info RITUAL_EVENTS

# Monitor stream messages
nats stream view RITUAL_EVENTS
```

### Event Monitoring
- **Event Streams** - Monitor `demon.ritual.v1.<ritualId>.<runId>.events`
- **Policy Decisions** - Watch for quota violations and denials
- **Approval Gates** - Track approval requests and resolutions

### Operate UI
- [Operate UI Guide](../operate-ui/README.md) - Web interface for monitoring
- **Runs Dashboard** - `/runs` endpoint for run visualization
- **API Endpoints** - `/api/runs` for programmatic access

## 🔧 Operational Procedures

### Routine Operations
- [Backup and Recovery](../ops/) - Data protection strategies
- [Scaling Considerations](../ops/) - Performance and capacity planning
- [Security Updates](../ops/) - Keeping systems secure

### Troubleshooting
- [Common Issues](../ops/) - Known problems and solutions
- [Log Analysis](../ops/) - Debugging techniques
- [Performance Tuning](../ops/) - Optimization strategies

### Maintenance Tasks
```bash
# Regular maintenance commands
make test                          # Verify system health
cargo run -p demonctl -- contracts bundle  # Export current contracts
nats stream info RITUAL_EVENTS    # Check stream health
nats stream info GRAPH_COMMITS    # Check graph commit stream health
```

### Graph Query Performance Considerations

**Replay Cost**: Graph query operations (`get-node`, `neighbors`, `path-exists`) replay the full commit history from genesis to the target commit to reconstruct graph state. For graphs with thousands of commits, expect query latency proportional to history depth.

**Operational Impact**:
- Small graphs (<100 commits): negligible latency
- Medium graphs (100-1000 commits): seconds to tens of seconds
- Large graphs (>1000 commits): consider snapshot-based optimization (future roadmap)

**Monitoring**: Track commit history depth via `nats stream info GRAPH_COMMITS` and correlate with query response times from `/api/graph/*` endpoints.

## 🔐 Security and Compliance

### Access Control
- **Bundle Verification** - Cryptographic signature validation
- **Policy Enforcement** - Automated quota and compliance checks
- **Approval Workflows** - Human gates for sensitive operations

### Audit and Compliance
- [Governance Documentation](../governance/) - Audit trails and controls
- [Contract Bundles](../contracts/releases.md) - Version tracking and integrity
- [Policy Decisions](../contracts/) - Compliance event logging

### Security Best Practices
- Never commit secrets or runtime data (`.demon/` directory)
- Use environment variables for sensitive configuration
- Verify bundle signatures before deployment
- Monitor approval patterns for anomalies

## 🏭 Production Patterns

### Multi-Tenant Setup
- **Stream Isolation** - Separate NATS streams per tenant
- **Policy Boundaries** - Per-tenant quota enforcement
- **Approval Isolation** - Tenant-specific approval workflows

### Integration Patterns
- **CI/CD Integration** - Automated deployment pipelines
- **Monitoring Integration** - External monitoring and alerting
- **Identity Provider Integration** - SSO and authentication

### Scaling Strategies
- **Horizontal Scaling** - Multiple runtime instances
- **Event Processing** - Distributed event handling
- **Storage Scaling** - NATS JetStream clustering

## 📋 Runbooks

### Emergency Procedures
- [Incident Response](../ops/) - Critical issue handling
- [Service Recovery](../ops/) - Restoration procedures
- [Data Recovery](../ops/) - Backup restoration

### Regular Maintenance
- [Health Check Schedule](../ops/) - Routine monitoring
- [Update Procedures](../ops/) - Safe update practices
- [Capacity Planning](../ops/) - Growth management

## 🤝 Integration

### API Integration
- [Approvals API](../../README.md#approvals-api) - REST endpoints for approval management
- [Event Streaming](../contracts/) - Consuming and producing events
- [Contract Management](../../README.md#contract-registry) - Schema and WIT integration

### External Systems
- **Monitoring Systems** - Prometheus, Grafana, DataDog integration
- **Logging Systems** - ELK, Splunk, CloudWatch integration
- **Notification Systems** - Slack, email, PagerDuty integration

## 📚 Reference

### Configuration Reference
- [Complete Environment Variables](../process/) - All configuration options
- [NATS JetStream Settings](../../docker/dev/) - Message system configuration
- [Policy Configuration](../contracts/) - Security and quota settings

### Architecture Reference
- [System Components](../../README.md#layout) - High-level architecture
- [Data Flow](../adr/) - How events and approvals work
- [Security Model](../adr/) - Trust boundaries and verification

---

**🚨 Need immediate help?** Check our [escalation procedures](../escalation-chains.md) or emergency runbooks.