# demonctl inspect — Graph Metrics Inspection

The `demonctl inspect` command provides real-time visibility into DAG node performance metrics, including queue lag, processing latency, and error rates. It surfaces scale hint recommendations to help operators understand system health and capacity needs.

## Quick Start

```bash
# View current graph metrics
demonctl inspect --graph

# Output machine-readable JSON
demonctl inspect --graph --json

# Inspect specific tenant
demonctl inspect --graph --tenant production

# Custom thresholds
demonctl inspect --graph \
  --warn-queue-lag 100 \
  --error-queue-lag 200 \
  --warn-p95-latency-ms 250.0 \
  --error-p95-latency-ms 500.0
```

## Prerequisites

The `inspect` command requires:
1. **Running NATS server** with JetStream enabled
2. **Runtime with scale hints enabled** (`SCALE_HINT_ENABLED=true`)
3. **SCALE_HINTS stream** configured in NATS
4. **Active workload** generating metrics

See [Scale Feedback Telemetry](scale-feedback.md) for runtime configuration.

## Command Reference

### Flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--graph` | boolean | `false` | Show graph metrics (DAG nodes, lag, latency, errors) |
| `--json` | boolean | `false` | Output machine-readable JSON |
| `--tenant` | string | `default` | Tenant ID to inspect (also from `DEMON_TENANT` env) |
| `--nats-url` | string | `nats://localhost:4222` | NATS server URL (also from `NATS_URL` env) |
| `--warn-queue-lag` | integer | `300` | Queue lag threshold for WARNING status |
| `--error-queue-lag` | integer | `500` | Queue lag threshold for ERROR status |
| `--warn-p95-latency-ms` | float | `500.0` | P95 latency threshold (ms) for WARNING |
| `--error-p95-latency-ms` | float | `1000.0` | P95 latency threshold (ms) for ERROR |
| `--warn-error-rate` | float | `0.02` | Error rate threshold (0.0-1.0) for WARNING |
| `--error-error-rate` | float | `0.05` | Error rate threshold (0.0-1.0) for ERROR |

### Environment Variables

All flags can be set via environment variables:

```bash
export DEMON_TENANT=production
export NATS_URL=nats://nats.example.com:4222
export INSPECT_WARN_QUEUE_LAG=300
export INSPECT_ERROR_QUEUE_LAG=500
export INSPECT_WARN_P95_LATENCY_MS=500.0
export INSPECT_ERROR_P95_LATENCY_MS=1000.0
export INSPECT_WARN_ERROR_RATE=0.02
export INSPECT_ERROR_ERROR_RATE=0.05

demonctl inspect --graph
```

## Output Modes

### Table Output (Default)

The default output mode renders a color-coded table with key metrics:

```
╭─────────┬────────┬───────────┬──────────────┬────────────┬─────────────────┬────────────────╮
│ TENANT  │ STATUS │ QUEUE LAG │ P95 LATENCY  │ ERROR RATE │ TOTAL PROCESSED │ RECOMMENDATION │
├─────────┼────────┼───────────┼──────────────┼────────────┼─────────────────┼────────────────┤
│ default │ WARN   │ 350       │ 650.5ms      │ 3.20%      │ 10000           │ ⬆ Scale Up     │
╰─────────┴────────┴───────────┴──────────────┴────────────┴─────────────────┴────────────────╯

Reason: Queue lag (350) exceeds warning threshold (300) and P95 latency (650.5ms) exceeds warning threshold (500ms)
Last Updated: 2025-01-06T15:30:45Z

Total Errors: 320 / 10000 (3.20%)

Status: WARN
```

#### Color Coding

Colors are automatically enabled for TTY output and disabled for pipes/redirects or when `NO_COLOR` environment variable is set:

- **OK** — Green text
- **WARN** — Yellow text
- **ERROR** — Red text
- **Scale Up** — Yellow arrow ⬆
- **Scale Down** — Blue arrow ⬇
- **Steady** — Green arrow ➡

**Disable colors:**
```bash
NO_COLOR=1 demonctl inspect --graph

# Or redirect to file (colors auto-disabled)
demonctl inspect --graph > metrics.txt
```

### JSON Output

Machine-readable JSON output suitable for automation and integration:

```bash
demonctl inspect --graph --json
```

**Sample output:**
```json
{
  "tenant": "default",
  "subject": "demon.scale.v1.default.hints",
  "status": "warn",
  "queue_lag": 350,
  "p95_latency_ms": 650.5,
  "error_rate": 0.032,
  "total_processed": 10000,
  "total_errors": 320,
  "last_error": null,
  "recommendation": "scale_up",
  "reason": "Queue lag (350) exceeds warning threshold (300) and P95 latency (650.5ms) exceeds warning threshold (500ms)",
  "timestamp": "2025-01-06T15:30:45Z"
}
```

#### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Graph Metrics Inspection Output",
  "type": "object",
  "required": [
    "tenant",
    "subject",
    "status",
    "queue_lag",
    "p95_latency_ms",
    "error_rate",
    "total_processed",
    "total_errors",
    "recommendation",
    "reason",
    "timestamp"
  ],
  "properties": {
    "tenant": {
      "type": "string",
      "description": "Tenant identifier"
    },
    "subject": {
      "type": "string",
      "description": "NATS subject where scale hints are published",
      "pattern": "^demon\\.scale\\.v1\\..+\\.hints$"
    },
    "status": {
      "type": "string",
      "enum": ["ok", "warn", "error"],
      "description": "Overall health status based on thresholds"
    },
    "queue_lag": {
      "type": "integer",
      "minimum": 0,
      "description": "Number of pending messages in queue"
    },
    "p95_latency_ms": {
      "type": "number",
      "minimum": 0,
      "description": "95th percentile processing latency in milliseconds"
    },
    "error_rate": {
      "type": "number",
      "minimum": 0.0,
      "maximum": 1.0,
      "description": "Error rate as decimal (0.0 = 0%, 1.0 = 100%)"
    },
    "total_processed": {
      "type": "integer",
      "minimum": 0,
      "description": "Total number of processed messages"
    },
    "total_errors": {
      "type": "integer",
      "minimum": 0,
      "description": "Total number of errors"
    },
    "last_error": {
      "type": ["string", "null"],
      "description": "Last error message (currently not populated)"
    },
    "recommendation": {
      "type": "string",
      "enum": ["scale_up", "scale_down", "steady"],
      "description": "Scaling recommendation based on current metrics"
    },
    "reason": {
      "type": "string",
      "description": "Human-readable explanation for the recommendation"
    },
    "timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "Timestamp of the scale hint event (ISO 8601)"
    }
  }
}
```

## Status Classification

The command classifies system health into three levels based on configurable thresholds:

### OK (Green)

All metrics are within acceptable ranges. System is operating normally.

**Criteria:**
- `queue_lag < warn_queue_lag`
- `p95_latency_ms < warn_p95_latency_ms`
- `error_rate < warn_error_rate`

### WARN (Yellow)

One or more metrics exceed warning thresholds but remain below error thresholds.

**Criteria (any true):**
- `queue_lag >= warn_queue_lag` AND `queue_lag < error_queue_lag`
- `p95_latency_ms >= warn_p95_latency_ms` AND `p95_latency_ms < error_p95_latency_ms`
- `error_rate >= warn_error_rate` AND `error_rate < error_error_rate`

### ERROR (Red)

One or more metrics exceed error thresholds. Immediate attention recommended.

**Criteria (any true):**
- `queue_lag >= error_queue_lag`
- `p95_latency_ms >= error_p95_latency_ms`
- `error_rate >= error_error_rate`

## Scale Recommendations

The system provides three types of scaling recommendations:

### ⬆ Scale Up

System is under pressure. Consider increasing agent capacity.

**Typical indicators:**
- High queue lag with messages backing up
- Elevated processing latency (P95 > threshold)
- Increasing error rates

### ⬇ Scale Down

System is underutilized. Consider reducing agent capacity for cost optimization.

**Typical indicators:**
- Low queue lag consistently
- Low processing latency
- Minimal errors

### ➡ Steady

System is operating within normal parameters. No scaling action needed.

## Error Handling

### Exit Codes

- **0** — Success, metrics retrieved and displayed
- **2** — Metrics unavailable (NATS unreachable, stream missing, or no data for tenant)
- **Other** — Unexpected error

### Common Error Scenarios

#### NATS Server Unreachable

```
Error: Scale hints unavailable: connection refused

Troubleshooting:
1. Ensure the runtime is running with SCALE_HINT_ENABLED=true
2. Check that NATS JetStream stream 'SCALE_HINTS' exists
3. Verify NATS_URL is correct: nats://localhost:4222
```

**Resolution:**
- Verify NATS is running: `nats server check`
- Check NATS URL: `echo $NATS_URL`
- Ensure runtime is configured to emit scale hints

#### No Scale Hints for Tenant

```
Error: No scale hints found for tenant 'production'

Troubleshooting:
1. Ensure the runtime is running and processing workload
2. Check that scale hints are being emitted (not in steady state)
3. Verify tenant ID is correct: production
```

**Resolution:**
- Check runtime logs for scale hint emission
- Verify tenant name matches runtime configuration
- Ensure workload is active (scale hints only emit when thresholds are crossed)

#### JSON Error Output

When using `--json`, errors are returned as JSON:

```json
{
  "error": "Scale hints unavailable: connection refused. Ensure SCALE_HINT_ENABLED=true and the runtime is emitting scale hints."
}
```

## Usage Examples

### Basic Inspection

```bash
# Default tenant with default thresholds
demonctl inspect --graph
```

### Production Monitoring

```bash
# Inspect production tenant with stricter thresholds
demonctl inspect --graph \
  --tenant production \
  --warn-queue-lag 100 \
  --error-queue-lag 200 \
  --warn-p95-latency-ms 200.0 \
  --error-p95-latency-ms 400.0 \
  --warn-error-rate 0.01 \
  --error-error-rate 0.02
```

### Automation Integration

```bash
# Export metrics as JSON for monitoring dashboard
demonctl inspect --graph --json > /var/metrics/graph-metrics.json

# Parse and alert based on status
STATUS=$(demonctl inspect --graph --json | jq -r '.status')
if [ "$STATUS" = "error" ]; then
  alert-pagerduty "Graph metrics in ERROR state"
fi
```

### CI/CD Health Checks

```bash
#!/bin/bash
# Pre-deployment health check

set -euo pipefail

echo "Checking graph metrics before deployment..."
METRICS=$(demonctl inspect --graph --json --tenant staging)

STATUS=$(echo "$METRICS" | jq -r '.status')
QUEUE_LAG=$(echo "$METRICS" | jq -r '.queue_lag')

if [ "$STATUS" = "error" ]; then
  echo "ERROR: Graph metrics show system under stress"
  echo "$METRICS" | jq .
  exit 1
fi

if [ "$QUEUE_LAG" -gt 100 ]; then
  echo "WARNING: Queue lag is elevated ($QUEUE_LAG), waiting..."
  sleep 30
fi

echo "✓ System healthy, proceeding with deployment"
```

### Watch Mode (External Tool)

```bash
# Monitor metrics with continuous refresh
watch -n 5 'demonctl inspect --graph'

# Or with color preservation
watch -n 5 --color 'demonctl inspect --graph'
```

### Remote NATS Monitoring

```bash
# Connect to remote NATS cluster
demonctl inspect --graph \
  --nats-url nats://nats.production.example.com:4222 \
  --tenant production
```

## Integration with Monitoring Systems

### Prometheus Exporter Pattern

```bash
#!/bin/bash
# prometheus-graph-metrics-exporter.sh
# Export graph metrics in Prometheus format

METRICS=$(demonctl inspect --graph --json)

QUEUE_LAG=$(echo "$METRICS" | jq -r '.queue_lag')
P95_LATENCY=$(echo "$METRICS" | jq -r '.p95_latency_ms')
ERROR_RATE=$(echo "$METRICS" | jq -r '.error_rate')
TOTAL_PROCESSED=$(echo "$METRICS" | jq -r '.total_processed')
TOTAL_ERRORS=$(echo "$METRICS" | jq -r '.total_errors')
STATUS=$(echo "$METRICS" | jq -r '.status')

cat <<EOF
# HELP demon_graph_queue_lag Number of pending messages
# TYPE demon_graph_queue_lag gauge
demon_graph_queue_lag{tenant="default"} $QUEUE_LAG

# HELP demon_graph_p95_latency_ms P95 processing latency in milliseconds
# TYPE demon_graph_p95_latency_ms gauge
demon_graph_p95_latency_ms{tenant="default"} $P95_LATENCY

# HELP demon_graph_error_rate Error rate as decimal
# TYPE demon_graph_error_rate gauge
demon_graph_error_rate{tenant="default"} $ERROR_RATE

# HELP demon_graph_total_processed Total messages processed
# TYPE demon_graph_total_processed counter
demon_graph_total_processed{tenant="default"} $TOTAL_PROCESSED

# HELP demon_graph_total_errors Total errors encountered
# TYPE demon_graph_total_errors counter
demon_graph_total_errors{tenant="default"} $TOTAL_ERRORS

# HELP demon_graph_status_ok System status OK (1=ok, 0=not ok)
# TYPE demon_graph_status_ok gauge
demon_graph_status_ok{tenant="default"} $([[ "$STATUS" == "ok" ]] && echo 1 || echo 0)

# HELP demon_graph_status_warn System status WARN (1=warn, 0=not warn)
# TYPE demon_graph_status_warn gauge
demon_graph_status_warn{tenant="default"} $([[ "$STATUS" == "warn" ]] && echo 1 || echo 0)

# HELP demon_graph_status_error System status ERROR (1=error, 0=not error)
# TYPE demon_graph_status_error gauge
demon_graph_status_error{tenant="default"} $([[ "$STATUS" == "error" ]] && echo 1 || echo 0)
EOF
```

**Usage:**
```bash
# Export metrics every 30 seconds
while true; do
  ./prometheus-graph-metrics-exporter.sh > /var/lib/node_exporter/textfile_collector/demon_graph.prom.tmp
  mv /var/lib/node_exporter/textfile_collector/demon_graph.prom.tmp \
     /var/lib/node_exporter/textfile_collector/demon_graph.prom
  sleep 30
done
```

### Grafana Dashboard Query Examples

```promql
# Queue lag over time
demon_graph_queue_lag{tenant="production"}

# P95 latency with threshold line
demon_graph_p95_latency_ms{tenant="production"}

# Error rate percentage
demon_graph_error_rate{tenant="production"} * 100

# Status indicator (0=OK, 1=WARN, 2=ERROR)
(demon_graph_status_warn * 1) + (demon_graph_status_error * 2)
```

## Best Practices

### Threshold Tuning

1. **Start with defaults** — Use built-in thresholds initially
2. **Monitor baselines** — Collect metrics for 24-48 hours to understand normal patterns
3. **Adjust gradually** — Change thresholds incrementally based on observed behavior
4. **Test under load** — Validate thresholds during peak traffic periods
5. **Document rationale** — Record why specific thresholds were chosen for your environment

### Operational Workflows

**Pre-deployment checks:**
```bash
# Verify system health before deploying
demonctl inspect --graph --tenant staging
# Manual review: STATUS should be "ok" or "steady"
```

**Incident response:**
```bash
# Quick diagnosis of performance issues
demonctl inspect --graph --tenant production

# Export full context for analysis
demonctl inspect --graph --json --tenant production > incident-$(date +%s).json
```

**Capacity planning:**
```bash
# Regular metrics collection for trend analysis
# Run via cron every 5 minutes:
# */5 * * * * demonctl inspect --graph --json --tenant prod >> /var/log/demon/capacity.jsonl
```

## Troubleshooting

### No Output After Running Command

**Symptom:** Command hangs without output

**Likely causes:**
- NATS server unreachable
- Network timeout
- Incorrect NATS URL

**Resolution:**
```bash
# Test NATS connectivity
nats server check --server=$NATS_URL

# Use shorter timeout with explicit URL
timeout 10s demonctl inspect --graph --nats-url nats://localhost:4222
```

### "Stream Not Found" Error

**Symptom:** Error about missing `SCALE_HINTS` stream

**Resolution:**
```bash
# Check if stream exists
nats stream ls

# Create stream if needed (usually handled by runtime)
nats stream add SCALE_HINTS \
  --subjects "demon.scale.v1.>" \
  --retention limits \
  --storage file
```

### Metrics Always Show "Steady"

**Symptom:** Recommendation is always "steady" even under load

**Likely causes:**
- Thresholds set too high
- Runtime not collecting metrics correctly
- Workload not generating enough traffic

**Resolution:**
```bash
# Lower thresholds temporarily to verify detection
demonctl inspect --graph \
  --warn-queue-lag 10 \
  --error-queue-lag 20

# Check runtime configuration
# Ensure SCALE_HINT_ENABLED=true in runtime environment
```

## See Also

- [Scale Feedback Telemetry](scale-feedback.md) — Runtime configuration and event schema
- [Operate UI Documentation](operate-ui/README.md) — Web-based metrics dashboard
- [NATS JetStream Documentation](https://docs.nats.io/jetstream) — Stream configuration reference

## Limitations

- **Single tenant query** — Currently inspects one tenant at a time; use multiple invocations for multi-tenant setups
- **Latest snapshot only** — Shows most recent scale hint; no historical trends (use Operate UI or export to TSDB)
- **No direct runtime API** — Queries NATS JetStream; future versions may support direct runtime API calls
- **last_error field** — Not currently populated; reserved for future enhancement

## Future Enhancements

- Per-node metrics breakdown (show individual DAG nodes)
- Historical trend queries with time ranges
- Multi-tenant summary view
- Direct runtime API integration (bypass NATS)
- Alert rule suggestions based on observed patterns
- Integration with external incident management systems
