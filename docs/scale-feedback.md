# Scale Hint Telemetry

The Demon runtime emits structured `agent.scale.hint:v1` events to provide autoscaling recommendations based on queue lag, processing latency, and error rates. These events feed into the Scale Hint Controller (Story #308) and Operate UI dashboard (Story #309).

## Overview

The scale hint system monitors runtime performance metrics and emits recommendations when thresholds are exceeded. It implements hysteresis-based state transitions to prevent scale oscillations and provides actionable insights for capacity planning.

## Event Schema

Events are published to JetStream subject `demon.scale.v1.<tenant>.hints` with the following structure:

```json
{
  "event": "agent.scale.hint:v1",
  "ts": "2025-01-06T10:30:02Z",
  "tenantId": "production",
  "recommendation": "scale_up",
  "metrics": {
    "queueLag": 850,
    "p95LatencyMs": 1250.5,
    "errorRate": 0.08,
    "totalProcessed": 1000,
    "totalErrors": 80
  },
  "thresholds": {
    "queueLagHigh": 500,
    "queueLagLow": 50,
    "p95LatencyHighMs": 1000.0,
    "p95LatencyLowMs": 100.0,
    "errorRateHigh": 0.05
  },
  "hysteresis": {
    "currentState": "overload",
    "stateChangedAt": "2025-01-06T10:29:00Z",
    "consecutiveHighSignals": 5,
    "consecutiveLowSignals": 0,
    "minSignalsForTransition": 3
  },
  "reason": "Queue lag (850) exceeds high threshold (500) and P95 latency (1250.5ms) exceeds high threshold (1000ms)",
  "traceId": "trace-scale-up-001"
}
```

### Recommendation Values

- `scale_up` — System is under pressure; consider increasing agent capacity
- `scale_down` — System is underutilized; consider reducing agent capacity
- `steady` — System is operating within normal parameters

### Pressure States

The hysteresis state machine transitions through three states to prevent flapping:

- `normal` — Metrics within acceptable range
- `pressure` — Elevated metrics; requires monitoring
- `overload` — Critical metrics; immediate action recommended

## Configuration

All configuration is done via environment variables:

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SCALE_HINT_ENABLED` | boolean | `false` | Enable scale hint telemetry |
| `SCALE_HINT_QUEUE_LAG_HIGH` | integer | `500` | Queue lag threshold for scale-up consideration |
| `SCALE_HINT_QUEUE_LAG_LOW` | integer | `50` | Queue lag threshold for scale-down consideration |
| `SCALE_HINT_P95_LATENCY_HIGH_MS` | float | `1000.0` | P95 latency threshold for scale-up (milliseconds) |
| `SCALE_HINT_P95_LATENCY_LOW_MS` | float | `100.0` | P95 latency threshold for scale-down (milliseconds) |
| `SCALE_HINT_ERROR_RATE_HIGH` | float | `0.05` | Error rate threshold (0.0-1.0) for scale-up |
| `SCALE_HINT_MIN_SIGNALS` | integer | `3` | Consecutive signals required before state transition |
| `SCALE_HINT_EMIT_ALL` | boolean | `false` | Emit events for all states (including steady) |

### Example Configuration

```bash
# Enable scale hints with custom thresholds
export SCALE_HINT_ENABLED=true
export SCALE_HINT_QUEUE_LAG_HIGH=1000
export SCALE_HINT_QUEUE_LAG_LOW=100
export SCALE_HINT_P95_LATENCY_HIGH_MS=2000.0
export SCALE_HINT_P95_LATENCY_LOW_MS=200.0
export SCALE_HINT_ERROR_RATE_HIGH=0.1
export SCALE_HINT_MIN_SIGNALS=5

# Start runtime
cargo run -p runtime
```

## Hysteresis Logic

The system uses hysteresis to prevent rapid oscillations in scale recommendations:

### State Transitions

1. **Normal → Pressure**: Requires `MIN_SIGNALS` consecutive high-pressure observations
2. **Pressure → Overload**: Requires `MIN_SIGNALS * 2` consecutive high-pressure observations
3. **Overload → Pressure**: Requires `MIN_SIGNALS` consecutive low-pressure observations
4. **Pressure → Normal**: Requires `MIN_SIGNALS` consecutive low-pressure observations

### High Pressure Signals

Any of the following conditions triggers a high-pressure signal:
- `queueLag > QUEUE_LAG_HIGH`
- `p95LatencyMs > P95_LATENCY_HIGH_MS`
- `errorRate > ERROR_RATE_HIGH`

### Low Pressure Signals

All of the following conditions must be true for a low-pressure signal:
- `queueLag < QUEUE_LAG_LOW`
- `p95LatencyMs < P95_LATENCY_LOW_MS`
- `errorRate < ERROR_RATE_HIGH`

## Testing & Simulation

A simulation utility is provided to generate sample scale hint events:

```bash
# Dry-run mode (no NATS publishing)
cargo run -p runtime --example simulate_scale_hints

# Publish to NATS
NATS_URL=nats://127.0.0.1:4222 cargo run -p runtime --example simulate_scale_hints -- --publish

# With custom configuration
SCALE_HINT_QUEUE_LAG_HIGH=800 \
SCALE_HINT_MIN_SIGNALS=2 \
NATS_URL=nats://127.0.0.1:4222 \
  cargo run -p runtime --example simulate_scale_hints -- --publish
```

The simulation runs three scenarios:
1. **Normal operation** — Steady-state metrics within thresholds
2. **Gradual pressure increase** — Demonstrates scale-up recommendation
3. **Recovery and low utilization** — Demonstrates scale-down recommendation

### Viewing Events in NATS

```bash
# Subscribe to all scale hint events
nats sub 'demon.scale.v1.>'

# Subscribe to specific tenant
nats sub 'demon.scale.v1.production.hints'

# Check stream info (if stream configured)
nats stream ls
nats stream info SCALE_HINTS
```

## Integration with Runtime

The scale hint emitter can be integrated into the runtime's main loop or triggered via API endpoints. Example integration:

```rust
use runtime::telemetry::{ScaleHintEmitter, ScaleHintConfig, RuntimeMetrics};

// Initialize emitter
let config = ScaleHintConfig::from_env();
let js_context = /* your JetStream context */;
let emitter = ScaleHintEmitter::new(config, Some(js_context), "default".to_string());

// Collect metrics (example)
let metrics = RuntimeMetrics {
    queue_lag: 650,
    p95_latency_ms: 1100.0,
    error_rate: 0.06,
    total_processed: 10000,
    total_errors: 600,
};

// Evaluate and emit
match emitter.evaluate_and_emit(metrics).await {
    Ok(Some(subject)) => {
        tracing::info!("Published scale hint to {}", subject);
    }
    Ok(None) => {
        tracing::debug!("No scale hint emitted (disabled or steady state)");
    }
    Err(e) => {
        tracing::error!("Failed to emit scale hint: {}", e);
    }
}
```

## Contract Validation

The event schema is validated in `engine/tests/agent_scale_hint_contracts_spec.rs`. Fixtures are available in `contracts/fixtures/events/`:
- `agent.scale.hint.scale_up.v1.json`
- `agent.scale.hint.scale_down.v1.json`
- `agent.scale.hint.steady.v1.json`

Run contract tests:
```bash
cargo test -p engine --test agent_scale_hint_contracts_spec
```

## Metrics Collection Implementation Notes

The current implementation provides the infrastructure for scale hint emission. To complete the integration, runtime implementers should:

1. **Collect queue lag metrics**: Track JetStream consumer lag or in-memory queue depth
2. **Track P95 latency**: Use histogram metrics (e.g., via the `metrics` crate) to compute 95th percentile
3. **Monitor error rates**: Track failed vs successful operations in a rolling window
4. **Periodically evaluate**: Call `emitter.evaluate_and_emit()` on a timer (e.g., every 30-60 seconds)

Example metrics collection pseudocode:

```rust
// In runtime main loop or background task
let mut interval = tokio::time::interval(Duration::from_secs(60));
loop {
    interval.tick().await;

    let metrics = RuntimeMetrics {
        queue_lag: get_consumer_lag().await?,
        p95_latency_ms: get_p95_latency(),
        error_rate: get_error_rate(),
        total_processed: get_total_processed(),
        total_errors: get_total_errors(),
    };

    let _ = emitter.evaluate_and_emit(metrics).await;
}
```

## Scale Hint Controller Service (Story #308)

The scale hint controller (`demon-scale-hint-handler`) consumes scale hint events from NATS JetStream and triggers autoscale actions.

### Running the Controller

```bash
# Dry-run mode (log-only, no autoscale calls)
DRY_RUN=true \
NATS_URL=nats://localhost:4222 \
cargo run -p scale-hint-handler --bin demon-scale-hint-handler

# With autoscale endpoint
DRY_RUN=false \
AUTOSCALE_ENDPOINT=http://kubernetes-hpa:8080/scale \
NATS_URL=nats://localhost:4222 \
cargo run -p scale-hint-handler --bin demon-scale-hint-handler
```

### Configuration Options

| Variable | Default | Description |
|----------|---------|-------------|
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `NATS_CREDS_PATH` | (none) | Path to NATS credentials file |
| `SCALE_HINT_STREAM_NAME` | `SCALE_HINTS` | JetStream stream name |
| `TENANT_FILTER` | (none) | Filter events for specific tenant (if not set, consumes all) |
| `DRY_RUN` | `true` | If true, logs only; if false, calls autoscale endpoint |
| `AUTOSCALE_ENDPOINT` | (none) | HTTP POST endpoint for autoscale actions |
| `CONSUMER_NAME` | `scale-hint-handler` | Durable consumer name |
| `RETRY_BACKOFF_MS` | `1000` | Initial retry backoff in milliseconds |
| `MAX_RETRY_ATTEMPTS` | `3` | Maximum retry attempts for autoscale calls |
| `AUTOSCALE_TIMEOUT_SECS` | `10` | Timeout for autoscale API calls |
| `METRICS_PORT` | `9090` | Port for metrics endpoint (currently log-based) |

### Deployment

The controller can be deployed as a standalone service or alongside the runtime. It maintains a durable JetStream consumer, ensuring at-least-once delivery with acknowledgment and retry logic.

#### Systemd

```ini
[Unit]
Description=Demon Scale Hint Handler
After=network.target nats.service

[Service]
Type=simple
ExecStart=/usr/local/bin/demon-scale-hint-handler
Environment="DRY_RUN=false"
Environment="AUTOSCALE_ENDPOINT=http://localhost:8080/scale"
Environment="NATS_URL=nats://localhost:4222"
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

#### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: demon-scale-hint-handler
spec:
  replicas: 1
  selector:
    matchLabels:
      app: scale-hint-handler
  template:
    metadata:
      labels:
        app: scale-hint-handler
    spec:
      containers:
      - name: handler
        image: demon/scale-hint-handler:latest
        env:
        - name: DRY_RUN
          value: "false"
        - name: AUTOSCALE_ENDPOINT
          value: "http://kube-hpa-scaler:8080/scale"
        - name: NATS_URL
          value: "nats://nats.nats-system:4222"
```

### Autoscale Endpoint Payload

When calling the autoscale endpoint, the controller POSTs the following JSON:

```json
{
  "tenant_id": "production",
  "recommendation": "scale_up",
  "metrics": {
    "queueLag": 850,
    "p95LatencyMs": 1250.5,
    "errorRate": 0.08,
    "totalProcessed": 1000,
    "totalErrors": 80
  },
  "reason": "Queue lag exceeds threshold and P95 latency is high",
  "timestamp": "2025-01-06T10:30:00Z",
  "traceId": "trace-12345"
}
```

### Known Limitations

- **Metrics**: Full Prometheus support pending resolution of metrics crate version conflict. Current implementation uses structured logging.
- **Testing**: One flaky retry test marked as ignored; core functionality verified by other tests.

## Future Enhancements

- **Full Prometheus metrics**: Resolve crate version conflict and add comprehensive metrics
- **Multi-dimensional scaling**: Consider additional metrics (CPU, memory, network)
- **Predictive scaling**: Use trend analysis for proactive recommendations
- **Custom policies**: Allow per-tenant threshold overrides
- **Webhook notifications**: Support external alerting systems

## References

- Contract Schema: `contracts/schemas/events.agent.scale.hint.v1.json`
- Telemetry Implementation: `runtime/src/telemetry/scale_hint.rs`
- Controller Service: `controller/scale-hint-handler/`
- Tests: `controller/scale-hint-handler/tests/integration_spec.rs`
- Contract Tests: `engine/tests/agent_scale_hint_contracts_spec.rs`
- Simulation: `examples/scale-hint/simulate.rs`
- Related Issues: #307 (telemetry), #308 (controller), #309 (UI dashboard)
