# Operate UI - Demon Meta-PaaS

A read-only web UI for monitoring and inspecting ritual runs in the Demon Meta-PaaS system.

## Runbook: Diagnosing Operate UI Template / Data Issues

Use this flow on a clean machine before filing an issue.

1) Is the UI healthy?

Start the UI without stream bootstrap so you see the empty-state path:

```
export DEMON_SKIP_STREAM_BOOTSTRAP=1
RUST_LOG=info cargo run -p operate-ui
```

Check the health report:

```
curl -s http://127.0.0.1:3000/admin/templates/report | jq .
```

You should see:

template_ready: true

has_filter_tojson: true

templates includes: base.html, runs_list.html, run_detail.html

If template_ready is false, see Common Causes below.

2) Expected behavior with no stream
```
curl -i http://127.0.0.1:3000/api/runs | sed -n '1,15p'
```

Expected:

HTTP/1.1 200 OK

Header X-Demon-Warn: JetStreamUnavailable (or similar)

Body {"runs":[]}

Visit http://localhost:3000/runs — you should see a banner:

“No event stream found. See Runbook: setup.”

3) Create the stream and seed two events

Defaults (override via env):

RITUAL_STREAM_NAME=RITUAL_EVENTS

RITUAL_SUBJECTS=demon.ritual.v1.>

Create stream and publish fixtures:

```
# Create stream (once per environment)
nats stream add $RITUAL_STREAM_NAME --subjects="$RITUAL_SUBJECTS" --retention=limits --storage=file --dupe-window=2m --discard=new

# Seed a run with 2 events
export RITUAL="echo-ritual"; export RUN="local-run-1"
nats pub "demon.ritual.v1.$RITUAL.$RUN.events" '{"event":"ritual.started:v1","ritualId":"'"$RITUAL"'","runId":"'"$RUN"'","ts":"2025-01-01T00:00:00Z"}' --header Nats-Msg-Id:"$RUN:1"
nats pub "demon.ritual.v1.$RITUAL.$RUN.events" '{"event":"ritual.completed:v1","ritualId":"'"$RITUAL"'","runId":"'"$RUN"'","ts":"2025-01-01T00:00:05Z","outputs":{"printed":"Hello"}}' --header Nats-Msg-Id:"$RUN:2"
```

Now:

```
curl -s http://127.0.0.1:3000/api/runs | jq .
curl -s http://127.0.0.1:3000/api/runs/$RUN | jq .
```

Visit http://localhost:3000/runs and http://localhost:3000/runs/$RUN — both should render without 500s.

Common Causes (and fixes)

Templates not found
Ensure crate-absolute glob is used; confirm /admin/templates/report lists the expected files. If not, re-run build and verify working dir.

Filter not registered
has_filter_tojson must be true. Filters are registered at boot in one place; if false, check startup logs.

Context shape drift (snake vs camel case)
Templates consume view-models with #[serde(rename_all="camelCase")]. If you see run.run_id in templates, replace with run.runId.

Missing stream
The API should return 200 with empty runs and a warning header. Create the stream (above) or run with auto-bootstrap enabled.

JetStream unavailable
You’ll still see /runs with a banner and /api/runs 200 + warning. Check your NATS/JetStream (make dev), then seed events.

## Overview

The Operate UI provides:
- **List View**: Recent ritual runs with status and basic information
- **Detail View**: Complete event timeline for individual runs
- **API Endpoints**: JSON APIs for programmatic access
- **Graceful Degradation**: Works even when JetStream is unavailable

## Features

- 📊 **Dashboard**: View recent runs at a glance
- 🔍 **Run Details**: Deep dive into event timelines
- 🚀 **Real-time Updates**: Auto-refresh for running rituals
- 📱 **Responsive Design**: Works on desktop and mobile
- 🛡️ **Error Handling**: Graceful fallbacks when services are unavailable
- 🔗 **API Access**: Full JSON API for integrations

## API Endpoints

### HTML Endpoints
- `GET /runs` - List recent runs (HTML)
- `GET /runs/:runId` - View run details (HTML)
- `GET /health` - Health check

### JSON API Endpoints
- `GET /api/runs?limit=50` - List recent runs (JSON)
- `GET /api/runs/:runId` - Get run details (JSON)

### API Response Formats

**List Runs Response:**
```json
[
  {
    "runId": "run-uuid",
    "ritualId": "my-ritual",
    "startTs": "2025-01-15T10:30:00Z",
    "status": "Completed"
  }
]
```

**Run Detail Response:**
```json
{
  "runId": "run-uuid",
  "ritualId": "my-ritual", 
  "events": [
    {
      "ts": "2025-01-15T10:30:00Z",
      "event": "ritual.started:v1",
      "stateFrom": null,
      "stateTo": "running"
    }
  ]
}
```

## Local Development

### Prerequisites

1. **Rust 1.82.0+** (configured in `rust-toolchain.toml`)
2. **NATS with JetStream** (optional - UI works without it)

### Setup

1. **Start NATS JetStream** (optional):
   ```bash
   make dev  # From project root
   ```

2. **Run the UI**:
   ```bash
   cd operate-ui
   cargo run
   ```

3. **Access the UI**:
   - Web UI: http://localhost:3000/runs
   - API: http://localhost:3000/api/runs
   - Health: http://localhost:3000/health

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | HTTP server port |
| `BIND_ADDR` | `0.0.0.0` | Bind address |
| `NATS_URL` | `nats://localhost:4222` | NATS server URL |
| `NATS_CREDS_PATH` | (none) | Path to NATS credentials file |

### Development with NATS

```bash
# Terminal 1: Start NATS
make dev

# Terminal 2: Run some rituals to generate data
cargo run --bin demonctl -- run examples/rituals/minimal.yaml

# Terminal 3: Start the UI
cd operate-ui && cargo run

# Terminal 4: View the results
curl http://localhost:3000/api/runs
open http://localhost:3000/runs
```

### Development without NATS

The UI gracefully handles NATS being unavailable:

```bash
# Just run the UI directly
cd operate-ui && cargo run
open http://localhost:3000/runs
```

You'll see friendly error messages indicating that JetStream is unavailable.

## Testing

### Run All Tests
```bash
cargo test
```

### Run Unit Tests Only
```bash
cargo test --test unit
```

### Run E2E Tests
```bash
cargo test --test e2e
```

### Run Integration Tests (requires NATS)
```bash
# Start NATS first
make dev

# Run integration tests
cargo test -- --ignored
```

### Test Coverage
```bash
# Install cargo-tarpaulin if needed
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out html
open tarpaulin-report.html
```

## Architecture

### Components

- **main.rs**: Server bootstrap and configuration
- **routes.rs**: HTTP handlers for HTML and JSON endpoints
- **jetstream.rs**: NATS JetStream client and data models
- **templates/**: Askama HTML templates

### Data Flow

1. **JetStream Events**: Ritual events are stored in JetStream on subjects like:
   ```
   demon.ritual.v1.<ritualId>.<runId>.events
   ```

2. **Query Layer**: The `jetstream.rs` module queries these subjects and transforms raw events into structured data models.

3. **HTTP Layer**: Routes in `routes.rs` handle incoming requests and render responses using either JSON serialization or HTML templates.

4. **UI Layer**: Askama templates provide responsive, accessible HTML with auto-refresh for running rituals.

### Error Handling

- **Service Unavailable**: When NATS/JetStream is down, the UI shows helpful error messages
- **Missing Data**: When runs don't exist, appropriate 404s are returned  
- **Timeouts**: All operations have defensive timeouts to prevent hanging
- **Graceful Degradation**: Core functionality works even with partial failures

## Deployment

### Docker (Future)
```dockerfile
FROM rust:1.82 as builder
COPY . .
RUN cargo build --release --bin operate-ui

FROM debian:bookworm-slim
COPY --from=builder /target/release/operate-ui /usr/local/bin/
EXPOSE 3000
CMD ["operate-ui"]
```

### Configuration for Production

```bash
export PORT=8080
export BIND_ADDR=0.0.0.0
export NATS_URL=nats://nats-cluster:4222
export NATS_CREDS_PATH=/etc/nats/creds/operate-ui.creds
export RUST_LOG=operate_ui=info
```

## Contributing

1. Follow the existing code style (rustfmt)
2. Add tests for new features
3. Update this README for significant changes
4. Ensure all tests pass: `cargo test`

## Security Considerations

- **Read-Only**: This UI only reads from JetStream, never writes
- **No Authentication**: Currently no auth - add reverse proxy if needed
- **Input Validation**: All user inputs are validated and sanitized
- **DoS Protection**: Limits on query sizes and timeouts prevent abuse
