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
nats stream info GRAPH_COMMITS

# Monitor message counts
nats stream view RITUAL_EVENTS --count

# Check graph storage resources
nats stream info GRAPH_COMMITS
nats kv info GRAPH_TAGS
```

### Common Operations
```bash
# Start development environment
make dev

# Run full smoke test
cargo run -p demonctl -- run examples/rituals/echo.yaml

# Check Operate UI health
curl http://localhost:3000/api/runs

# Access Schema Form Renderer
open http://localhost:3000/ui/form

# Load a local schema in the form renderer
open "http://localhost:3000/ui/form?schemaName=approval.requested.v1"

# Export current contracts
cargo run -p demonctl -- contracts bundle
```

### Schema Form Renderer Operations

The Schema Form Renderer provides a web-based interface for rendering JSON Schema Draft 2020-12 compliant schemas into accessible HTML forms.

**Access the form renderer:**
```bash
# Via browser
open http://localhost:3000/ui/form

# With a specific local schema
open "http://localhost:3000/ui/form?schemaName=approval.requested.v1"

# With a remote schema URL
open "http://localhost:3000/ui/form?schemaUrl=https://example.com/schema.json"
```

**Features:**
- **Local schema loading** - Load schemas from `contracts/schemas/`
- **Remote schema loading** - Fetch schemas from URLs
- **Form validation** - Draft 2020-12 schema validation
- **Accessible design** - WCAG-compliant form controls with ARIA labels
- **Live updates** - `form.changed` events emitted on every field change
- **JSON preview** - View form data as JSON in real-time

**Supported schema features:**
- Basic types: string, number, integer, boolean, object
- Formats: date-time, email, uri
- Constraints: required, min/max, pattern, enum
- Nested objects (limited array support)

**API endpoints:**
- `GET /ui/form` - Form renderer page
- `GET /api/schema/metadata` - Fetch schema metadata
- `POST /api/form/submit` - Submit form data

**Example:**
```bash
# Load approval request schema
curl "http://localhost:3000/api/schema/metadata?schemaName=approval.requested.v1" | jq .

# Submit form data
curl -X POST http://localhost:3000/api/form/submit \
  -H "Content-Type: application/json" \
  -d '{"schemaId": "test", "data": {"field": "value"}}'
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

# Query graph: get commit by ID
curl "http://localhost:8080/api/graph/commits/<COMMIT_ID>?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# Query graph: list commits
curl "http://localhost:8080/api/graph/commits?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1&limit=50"

# Query graph: get tag
curl "http://localhost:8080/api/graph/tags/v1.0.0?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# Query graph: list all tags
curl "http://localhost:8080/api/graph/tags?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# Verify graph events in NATS JetStream
nats stream info GRAPH_COMMITS
nats stream view GRAPH_COMMITS --count
```

### Viewing Graphs in Operate

The Operate UI provides a minimal read-only graph viewer for inspecting graph commits and tags.

**Access the graph viewer:**
```
http://localhost:3000/graph
```

**Features:**
- Input form to specify graph scope (tenant, project, namespace, graph ID)
- List of commits with timestamps, parent commits, and mutation counts
- List of tags with associated commit IDs
- Click on any commit to view detailed mutation JSON
- Auto-loads on page load with default scope parameters

**Query Parameters:**
```
http://localhost:3000/graph?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
```

**Environment Configuration:**
- `RUNTIME_API_URL` - Runtime server URL (default: `http://localhost:8080`)

**Note:** The viewer calls the Runtime REST endpoints at `/api/graph/commits` and `/api/graph/tags`. Ensure the runtime server is running and accessible.

### Workflow Viewer

The Workflow Viewer provides a visual representation of Serverless Workflow definitions with support for live state updates via SSE.

**Access the workflow viewer:**
```bash
# Via browser
open http://localhost:3000/ui/workflow

# With a specific local workflow (relative to examples/rituals/)
open "http://localhost:3000/ui/workflow?workflowPath=echo.yaml"

# With a remote workflow URL
open "http://localhost:3000/ui/workflow?workflowUrl=https://example.com/workflow.yaml"
```

**Features:**
- **Local workflow loading** - Load workflows from `examples/rituals/` directory
- **Remote workflow loading** - Fetch workflows from URLs (with 1MB size limit and 10s timeout)
- **YAML/JSON parsing** - Supports both YAML and JSON workflow formats
- **State visualization** - Displays workflow tasks/states with visual indicators
- **Live updates** - SSE support for real-time state highlighting (infrastructure for future implementation)
- **Accessible design** - WCAG-compliant with ARIA labels and keyboard navigation
- **Minimal bundle** - Under 5 KB gzipped (well below 150 KB budget)
- **Pause/resume streaming** - Control SSE connection state

**Supported workflow formats:**
- **CNCF Serverless Workflow 1.0** - `document.do` task definitions
- **Legacy formats** - `states` array definitions

**Supported task types:**
call, do, emit, for, fork, listen, raise, run, set, switch, try, wait

**State visualization:**
- **Pending** - Gray outline (not yet started)
- **Running** - Blue with pulse animation (currently executing)
- **Waiting** - Orange with pulse (awaiting event or time)
- **Completed** - Green (successfully finished)
- **Faulted** - Red (encountered error)
- **Suspended** - Orange (paused by user)

**API endpoints:**
- `GET /ui/workflow` - Workflow viewer page
- `GET /api/workflow/metadata` - Fetch workflow YAML/JSON
- `GET /api/workflow/state` - Get current execution state (placeholder)

**Security:**
- Path traversal protection (sanitizes `..` in paths)
- Size limits: 1 MB max for workflow files
- Timeout limits: 10 seconds for remote HTTP fetches
- Safe HTML escaping in rendered output

**Example:**
```bash
# Load echo ritual workflow
curl "http://localhost:3000/api/workflow/metadata?workflowPath=echo.yaml" | jq .

# Load timer workflow
open "http://localhost:3000/ui/workflow?workflowPath=timer.yaml"

# View workflow state (placeholder API)
curl "http://localhost:3000/api/workflow/state?workflowId=echo-ritual" | jq .
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
  - `RITUAL_EVENTS` stream: ritual execution events
  - `GRAPH_COMMITS` stream: graph commit events with replay support
  - `GRAPH_TAGS` bucket: KV store for graph tag lookups
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