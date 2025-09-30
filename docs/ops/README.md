# Operations Documentation

![Status: Current](https://img.shields.io/badge/Status-Current-green)

This directory contains operational procedures, runbooks, and troubleshooting guides for running Demon in production.

## Overview

Operations documentation provides the knowledge and procedures needed to deploy, monitor, and maintain Demon in production environments.

## Runbook Index

### Deployment & Setup
| Procedure | Purpose | Complexity | Last Updated |
|-----------|---------|------------|--------------|
| [Bootstrapper Setup](../bootstrapper/) | Initial cluster deployment | Intermediate | Current |
| [Development Environment](../tutorials/alpha-preview-demo.md) | Local dev setup | Beginner | Current |
| *Production Deployment* | *Production deployment guide* | *Advanced* | *Coming Soon* |

### Monitoring & Health Checks
| Procedure | Purpose | Complexity | Last Updated |
|-----------|---------|------------|--------------|
| [Health Check Endpoints](../api/README.md) | Service health verification | Beginner | Current |
| *Metrics & Alerting* | *Monitoring setup* | *Intermediate* | *Coming Soon* |
| *Log Aggregation* | *Centralized logging* | *Advanced* | *Coming Soon* |

### Troubleshooting
| Issue Category | Common Problems | Quick Fixes | Escalation |
|----------------|----------------|-------------|------------|
| **Runtime Issues** | Pod crashes, memory leaks | Restart services, check logs | Platform team |
| **Network Issues** | Connection failures, timeouts | Check connectivity, DNS | Network team |
| **Storage Issues** | Disk full, permission errors | Clean logs, check mounts | Storage team |
| **Integration Issues** | NATS connection, event delivery | Verify NATS, check queues | Integration team |

### Maintenance
| Task | Frequency | Complexity | Owner |
|------|-----------|------------|--------|
| **Log Rotation** | Daily | Beginner | Ops |
| **Backup Verification** | Weekly | Intermediate | Ops |
| **Security Updates** | Monthly | Advanced | Security + Ops |
| **Capacity Planning** | Quarterly | Advanced | Platform team |

Operations documentation helps platform engineers and operators maintain healthy Demon deployments through:

- **Runbooks** - Step-by-step operational procedures
- **Troubleshooting** - Common issues and resolution steps
- **Monitoring** - Observability and alerting strategies
- **Maintenance** - Routine care and updates

## Quick Reference

### Emergency Procedures
- **Service Down** - See [escalation chains](../escalation-chains.md)
- **Data Loss** - Follow backup restoration procedures
- **Performance Issues** - Check resource utilization and NATS health
- **Security Incidents** - Isolate, assess, and remediate

### Health Checks
```bash
# Verify system health
cargo run -p demonctl -- bootstrap --verify

# Check NATS stream status
nats stream info RITUAL_EVENTS

# Monitor message counts
nats stream view RITUAL_EVENTS --count
```

### Common Operations
```bash
# Start development environment
make dev

# Run full smoke test
cargo run -p demonctl -- run examples/rituals/echo.yaml

# Check Operate UI health
curl http://localhost:3000/api/runs

# Export current contracts
cargo run -p demonctl -- contracts bundle
```

### Graph Capsule Operations
```bash
# Create a new graph with seed mutations
demonctl graph create \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  mutations.json

# Commit mutations to an existing graph
demonctl graph commit \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  --parent-ref <COMMIT_ID> \
  mutations.json

# Tag a commit
demonctl graph tag \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  --tag v1.0.0 \
  --commit-id <COMMIT_ID>

# List tags for a graph
demonctl graph list-tags \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1

# Verify graph events in NATS JetStream
nats stream info GRAPH_COMMITS
nats stream view GRAPH_COMMITS --count
```

## Runbooks

### Daily Operations
- **Health Monitoring** - Automated checks and manual verification
- **Log Review** - Error patterns and performance metrics
- **Backup Verification** - Ensure data protection strategies
- **Capacity Planning** - Monitor growth and resource usage

### Weekly Operations
- **Security Updates** - Apply patches and updates
- **Performance Review** - Analyze trends and optimize
- **Documentation Updates** - Keep runbooks current
- **Team Training** - Knowledge sharing and skill building

### Monthly Operations
- **Disaster Recovery Testing** - Validate backup and restore
- **Capacity Forecasting** - Plan for growth and scaling
- **Security Audit** - Review access and compliance
- **Architecture Review** - Assess and plan improvements

## Monitoring & Alerting

### Key Metrics
- **Event Processing Rate** - Messages per second through NATS
- **Approval Response Time** - Time from request to resolution
- **Error Rates** - Failed rituals and system errors
- **Resource Utilization** - CPU, memory, and storage usage

### Alert Conditions
- **Stream Lag** - Events backing up in NATS JetStream
- **Approval Timeouts** - TTL auto-deny rate increasing
- **High Error Rate** - Ritual failures above threshold
- **Resource Exhaustion** - System resources approaching limits

### Integration Points
- **NATS Monitoring** - JetStream health and performance
- **Application Logs** - Structured logging for observability
- **External Monitoring** - Prometheus, Grafana, DataDog
- **Notification Systems** - Slack, email, PagerDuty

## Troubleshooting

### Common Issues

#### NATS Connection Problems
```bash
# Check NATS server status
nats server check

# Verify stream configuration
nats stream info RITUAL_EVENTS

# Test connection
nats pub test.subject "test message"
```

#### Approval Gate Issues
```bash
# Check pending approvals
curl http://localhost:3000/api/runs?status=awaiting_approval

# Review approval logs
grep "approval" logs/demon.log

# Verify TTL worker
nats consumer info RITUAL_EVENTS ttl-worker
```

#### Performance Issues
```bash
# Check system resources
top
df -h

# Monitor NATS performance
nats server info

# Review application metrics
curl http://localhost:3000/metrics
```

### Log Analysis
```bash
# Application logs
tail -f logs/demon.log

# NATS server logs
tail -f /var/log/nats/nats.log

# System logs
journalctl -u demon -f
```

## Maintenance

### Regular Tasks
- **Log Rotation** - Prevent disk space issues
- **Certificate Renewal** - TLS certificate management
- **Dependency Updates** - Security patches and bug fixes
- **Configuration Review** - Ensure settings are optimal

### Backup & Recovery
- **NATS Stream Backup** - Event data protection
- **Configuration Backup** - System settings preservation
- **Testing Recovery** - Validate restoration procedures
- **Documentation Updates** - Keep procedures current

### Scaling Operations
- **Horizontal Scaling** - Multiple runtime instances
- **Vertical Scaling** - Resource allocation increases
- **Load Balancing** - Traffic distribution strategies
- **Cache Optimization** - Performance improvements

## Security Operations

### Access Management
- **User Provisioning** - Account creation and permissions
- **Key Rotation** - Regular credential updates
- **Audit Logging** - Security event monitoring
- **Compliance Reporting** - Regulatory requirements

### Incident Response
- **Detection** - Automated and manual monitoring
- **Containment** - Isolate and limit impact
- **Investigation** - Root cause analysis
- **Recovery** - Service restoration and lessons learned

---

**🚨 Emergency?** Check [escalation procedures](../escalation-chains.md) or contact on-call team.

**🔗 Related**: [Platform Engineers Guide](../personas/operators.md) | [Escalation Chains](../escalation-chains.md) | [Governance](../governance/)