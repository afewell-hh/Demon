# Operate UI Quickstart

A read‑only UI for viewing ritual runs and their event timelines from NATS JetStream.

## Prerequisites
- Rust 1.82.0 (see ADR-0002).
- Docker (for local NATS JetStream).

## Run Locally
```bash
make dev                         # starts NATS JetStream on 4222/8222
cargo build --workspace
cargo run -p operate-ui          # starts the UI server
```

Then visit:
- `/runs` — recent runs, stable ordering (legacy - defaults to tenant "default")
- `/runs/:runId` — ordered timeline per run (legacy - defaults to tenant "default")
- `/graph` — graph viewer for commits, tags, and DAG visualization
- `/ui/contracts` — contracts browser (feature-flagged, see Feature Flags section)
- `/api/runs`, `/api/runs/:runId` — JSON APIs (502 when NATS unavailable) (legacy - defaults to tenant "default")

## Feature Flags

The Operate UI supports feature flags to enable experimental or in-development features.

### Configuration

Feature flags can be enabled via:
- **Environment variable**: `OPERATE_UI_FLAGS=feature1,feature2`
- **Query parameter**: `?flags=feature1,feature2`

### Available Flags

- `contracts-browser` — Enables the Contracts Browser UI for viewing and searching schema registry contracts

### Example

```bash
# Enable via environment variable
export OPERATE_UI_FLAGS=contracts-browser
cargo run -p operate-ui

# Enable via query parameter (useful for testing)
curl http://localhost:3000/ui/contracts?flags=contracts-browser
```

## Multi-tenant Support

The UI now supports tenant namespace isolation for multi-tenant deployments:

### Tenant-aware API Endpoints
- `/api/tenants/:tenant/runs` — list runs for a specific tenant
- `/api/tenants/:tenant/runs/:runId` — get run detail for a specific tenant
- `/api/tenants/:tenant/runs/:runId/events/stream` — SSE stream for a specific tenant's run
- `/api/tenants/:tenant/approvals/:runId/:gateId/grant` — grant approval for a specific tenant
- `/api/tenants/:tenant/approvals/:runId/:gateId/deny` — deny approval for a specific tenant

### JetStream Subject Pattern
Events are now published to tenant-scoped subjects:
- New pattern: `demon.ritual.v1.<tenant>.<ritualId>.<runId>.events`
- Legacy pattern: `demon.ritual.v1.<ritualId>.<runId>.events` (backward compatible with tenant "default")

## Notes
- Read-only semantics: ephemeral consumers; no durable state created by the UI.
- Deterministic fetch: multi-batch reads until a short batch; no hangs.
- Failure mode: if NATS is down, HTML pages render a friendly error; APIs return 502.
- Review protocol: open PR as Draft, satisfy the Evidence Checklist, then freeze at a commit SHA for review.
- Stream selection: set `RITUAL_STREAM_NAME` (default `RITUAL_EVENTS`). If absent, the UI will fall back to the legacy `DEMON_RITUAL_EVENTS` stream and log a deprecation warning.

## Live Event Streaming

The UI now supports real-time event streaming via Server-Sent Events (SSE):

### Features
- **Real-time updates**: Run detail pages (`/runs/:runId`) automatically update as new events arrive from JetStream
- **Connection status indicator**: Visual badge showing connection state (Connected, Reconnecting, Offline)
- **Automatic reconnection**: Exponential backoff with jitter for robust reconnection handling
- **Graceful degradation**: Falls back to heartbeat-only mode when JetStream is unavailable

### Configuration
- `SSE_HEARTBEAT_SECONDS`: Interval for keepalive heartbeats (default: 15 seconds)
- `SSE_RETRY_BASE_MS`: Base delay for reconnection backoff (default: 250ms)
- `SSE_RETRY_MAX_MS`: Maximum delay for reconnection backoff (default: 5000ms)

### SSE Endpoint
- **Path**: `/api/runs/:runId/events/stream`
- **Events**:
  - `init`: Initial connection established
  - `append`: New event to add to timeline
  - `heartbeat`: Keepalive signal
  - `warning`: JetStream unavailable, degraded mode
  - `error`: Stream error occurred

### Behavior
1. On page load, establishes SSE connection to stream endpoint
2. Receives initial snapshot of existing events (via `init` event)
3. Tails JetStream for new events using ephemeral consumer with `DeliverPolicy::New`
4. Updates DOM in real-time as events arrive:
   - Inserts new events in chronological order
   - Updates run status badges
   - Updates event count
   - Briefly highlights new events
5. Maintains connection with periodic heartbeats
6. Automatically reconnects on disconnection with exponential backoff

## Approval TTL

- Env: `APPROVAL_TTL_SECONDS` (default `0`, disabled). Example: `export APPROVAL_TTL_SECONDS=5`.
- Behavior: when `approval.requested:v1` is appended (via engine hook), a timer is scheduled for `requested_ts + TTL` with ID `"{runId}:approval:{gateId}:expiry"`.
- On expiry: if no terminal exists for the gate, the system appends `approval.denied:v1` with `{"reason":"expired","approver":"system"}` using idempotency key `"{runId}:approval:{gateId}:denied"`.
- UI: shows status as `Denied — expired` when the denial reason is `expired`.

## TTL Worker (approvals expiry)

- Start: `TTL_WORKER_ENABLED=1 cargo run -p engine --bin demon-ttl-worker`
- Env:
  - `NATS_URL` (default `nats://127.0.0.1:4222`)
  - `RITUAL_STREAM_NAME` (optional; else `RITUAL_EVENTS` then `DEMON_RITUAL_EVENTS`)
  - `TTL_CONSUMER_NAME` (default `ttl-worker`), `TTL_BATCH` (100), `TTL_PULL_TIMEOUT_MS` (1500)
- Behavior: consumes `timer.scheduled:v1` on `demon.ritual.v1.*.*.events`, calls auto-expiry, acks on success/no-op.
- Monitoring: logs `ttl_worker` events and in-process counters.

## Preview Mode

- See docs/preview/alpha/runbook.md for a 10‑minute, one‑command demo.
 - After starting the UI and TTL worker, run `./examples/seed/seed_preview.sh` and open `/runs`.
- To seed for a specific tenant: `TENANT=acme ./examples/seed/seed_preview.sh`
- Preview Mode links:
   - Runbook (One‑Pager): `docs/preview/alpha/runbook.md`
   - Client Deck (5 slides): `docs/preview/alpha/deck.md`
   - Presenter Script (60‑sec): `docs/preview/alpha/presenter_script.md`
  - Dry‑Run Checklist: `docs/preview/alpha/dry_run_checklist.md`

## Workflow Viewer

The Operate UI includes an enhanced workflow viewer for visualizing and managing Serverless Workflow 1.0 definitions:

### Features
- **Workflow Discovery**: Browse all available workflows from the `examples/rituals/` directory
- **Search & Filter**: Real-time search by workflow name or description
- **Manual Load**: Load workflows by local path or remote URL
- **Visual Rendering**: Display workflow tasks/states with current execution status
- **State Updates**: Polling-based state updates (5-second intervals) for active workflows
- **YAML Inspection**: View raw workflow YAML/JSON definitions

### Endpoints
- `/ui/workflow` — Workflow viewer UI page
- `/api/workflows` — List available local workflows (JSON array)
- `/api/workflow/metadata?workflowPath=<path>` — Get workflow metadata by local path
- `/api/workflow/metadata?workflowUrl=<url>` — Get workflow metadata by remote URL
- `/api/workflow/state?workflowId=<id>` — Get current workflow execution state (placeholder)

### Usage

#### Browse Workflows
1. Navigate to `/ui/workflow`
2. Click "Browse Workflows" button
3. Use the search box to filter workflows by name or description
4. Click "View" on any workflow to load and visualize it

#### Manual Load
1. Navigate to `/ui/workflow`
2. Enter a local path (relative to `examples/rituals/`) or remote URL
3. Click "Load Workflow" to fetch and display the workflow

### Example Workflows
- `echo.yaml` — Simple echo ritual demonstrating basic task execution
- `timer.yaml` — Timer-based workflow example

### Live Updates via SSE

The workflow viewer now supports real-time state updates via Server-Sent Events (SSE):

#### Features
- **Real-time commit streaming**: Connects to runtime's graph commit SSE endpoint
- **Automatic reconnection**: Exponential backoff (1s → 30s max) with 10 retry limit
- **Polling fallback**: Falls back to 5-second polling after max retries
- **Connection status**: Visual indicator showing Connected/Reconnecting/Polling/Paused states
- **Stream control**: Pause/Resume button to control SSE connection

#### Configuration
- Runtime SSE endpoint: `${RUNTIME_API_URL}/api/graph/commits/stream`
- Heartbeat interval: Configurable via `SSE_HEARTBEAT_SECONDS` (default: 25s in runtime)
- Reconnection policy:
  - Max retries: 10 attempts
  - Backoff: 1s, 2s, 4s, 8s, 16s (capped at 30s)
  - After max retries: switches to 5s polling

#### Event Handling
- `init`: Receives initial snapshot of recent commits
- `commit`: Processes new graph commits and updates task states based on mutations
- `heartbeat`: Maintains connection liveness
- `warning`: Logs non-fatal issues (e.g., snapshot load failure)
- `error`: Triggers reconnection logic

#### Implementation
Connection established via `EventSource` API when workflow is loaded. Graph commit mutations are processed to extract workflow state changes (e.g., task state transitions). Task visuals update in real-time as mutations arrive.

### Future Enhancements
- **Approval Gates**: Display and interact with workflow approval gates
- **Policy Decisions**: Show policy evaluation results alongside task states
- **Runtime Integration**: Deep integration with runtime API for execution control
- **Multi-graph filtering**: Filter SSE stream by specific graphId

## Admin Probe (dev-only)

- Endpoint: `/admin/templates/report` returns JSON `{ template_ready, has_filter_tojson, templates }` used by the bootstrapper verify phase.
- Optional auth: set `ADMIN_TOKEN` in the environment to require header `X-Admin-Token: <token>`; without it, the probe is unauthenticated (dev-only).
- Admin: `/admin/templates/report` shows `template_ready=true` and `has_filter_tojson=true`.

## Graph Viewer

The Operate UI provides a web-based graph viewer at `/graph` for visualizing graph commits, tags, and the commit DAG.

### Features
- **Commit History**: Browse recent commits with metadata (parent, timestamp, mutation count)
- **Tag Management**: View all tags and their associated commits
- **Live Updates**: SSE integration for real-time commit/tag notifications
- **Filtering**: Filter commits by text search or mutation type (add-node, add-edge, etc.)
- **DAG Visualization**: Interactive SVG-based commit graph showing parent-child relationships
- **Commit Details**: Drill down into individual commits to view full mutation payloads

### Usage
Navigate to `/graph`:
```
http://localhost:3000/graph
```

Query parameters allow pre-populating graph scope:
```
http://localhost:3000/graph?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
```

See `docs/api/graph.md` for detailed API documentation and usage examples.

## Contracts Browser

The Contracts Browser provides a web-based interface for exploring the schema registry.

### Features
- **Contract Discovery**: Browse all available contracts from the schema registry
- **Search & Filter**: Real-time search by contract name, version, or author
- **Detail View**: View full contract metadata, JSON schemas, and WIT definitions
- **Schema Preview**: Display schema excerpts (first 40 lines) with expand toggle
- **Download**: Export contract schemas as JSON files
- **Accessibility**: Keyboard navigation (Escape to close drawer), ARIA roles

### Endpoints
- `/ui/contracts` — Contracts Browser UI page (feature-flagged)
- `/api/contracts/registry/list` — List all contracts (JSON array)
- `/api/contracts/registry/:name/:version` — Get specific contract bundle

### Configuration
- **Feature Flag**: Requires `contracts-browser` flag to be enabled
- **Registry URL**: Set `SCHEMA_REGISTRY_URL` (default: `http://localhost:8080`)

### Usage

```bash
# Enable feature flag
export OPERATE_UI_FLAGS=contracts-browser
export SCHEMA_REGISTRY_URL=http://localhost:8080

# Start UI
cargo run -p operate-ui

# Visit in browser
open http://localhost:3000/ui/contracts?flags=contracts-browser
```

### User Experience
1. Navigate to Contracts Browser from the navigation menu
2. View list of available contracts with metadata
3. Use search box to filter by name, version, or author
4. Click "View" to open detail drawer with full schema
5. Click "Download Schema" to export as JSON
6. Press Escape to close drawer

### Error Handling
- **404**: Feature flag not enabled
- **502 Bad Gateway**: Schema registry unavailable
- **Empty state**: No contracts in registry or no matches for search

## Canvas UI

The Canvas UI provides an interactive DAG (Directed Acyclic Graph) visualization of ritual execution flows with real-time telemetry overlays.

### Features
- **Real-time DAG rendering**: Force-directed graph layout powered by D3.js v7
- **Node type visualization**: Distinct colors for rituals, capsules, streams, gates, UI endpoints, policies, and infrastructure
- **Live telemetry overlays**: Color-coded lag/latency metrics on edges (green/amber/red thresholds)
- **Interactive node inspector**: Click nodes to view metadata, contract links, and execution status
- **Navigation controls**: Zoom (+/−), pan (drag), reset view, pause/resume simulation
- **Minimap**: Overview panel with viewport indicator for large graphs
- **Keyboard accessibility**: Escape to close inspector, Tab navigation, Enter/Space activation
- **Connection status**: Real-time indicator showing Connected/Reconnecting/Offline states

### Configuration

**Feature Flag**: Requires `canvas-ui` flag to be enabled

```bash
# Enable Canvas UI
export OPERATE_UI_FLAGS=canvas-ui

# Enable multiple features (comma-separated)
export OPERATE_UI_FLAGS=canvas-ui,contracts-browser

# Start UI
cargo run -p operate-ui

# Navigate to Canvas
open http://localhost:3030/canvas
```

### Endpoints

- `/canvas` — Canvas UI page (feature-flagged, returns 404 when disabled)
- `/api/canvas/graph?tenant=<tenant>&run_id=<run_id>` — Get graph data (future)
- `/api/canvas/telemetry/stream` — SSE stream for live telemetry updates (future)

### Node Types

| Node Type | Color | Description |
|-----------|-------|-------------|
| Ritual | Blue | Top-level workflow orchestrator |
| Capsule | Green | WebAssembly execution unit |
| Stream | Orange | NATS JetStream event stream |
| Gate | Purple | Approval gate requiring human decision |
| UI Endpoint | Cyan | Operate UI exposure point |
| Policy | Red | Policy ward for validation/guardrails |
| Infrastructure | Blue-Grey | Supporting infrastructure (NATS, etc.) |

### Telemetry Thresholds

Edge telemetry uses color coding to indicate performance:

| Threshold | Color | Latency Range | Interpretation |
|-----------|-------|---------------|----------------|
| Healthy | Green | < 50ms | Normal operation |
| Warning | Amber | 50ms - 150ms | Elevated latency |
| Critical | Red | > 150ms | Performance degradation |

### User Interface

#### Main Canvas Area
- SVG canvas with force-directed graph layout
- Curved edges with directional arrows
- Telemetry badges showing lag/latency values
- Auto-stabilizing force simulation

#### Node Inspector Panel
Clicking a node opens a slide-out panel showing:
- Node ID and type
- Current execution status
- Contract link (navigates to `/ui/contracts` for schema details)
- Metadata (varies by node type)

**Keyboard Navigation:**
- `Escape` — Close inspector
- `Tab` / `Shift+Tab` — Navigate between interactive elements

#### Controls Toolbar
- **Zoom In** (+) — Increase magnification
- **Zoom Out** (−) — Decrease magnification
- **Reset View** (⟳) — Return to default zoom/pan
- **Pause/Resume** (⏸/▶) — Freeze/unfreeze force simulation
- **Connection Status** — Shows "Connected", "Reconnecting", or "Offline"

### Current Implementation

The initial implementation includes:
- **Mock data**: Embedded sample graph representing a typical ritual execution
- **Static rendering**: Force-directed layout with simulated telemetry updates (1s interval)
- **Connection simulation**: Toggles offline/reconnecting states every 30 seconds
- **Feature flag gating**: Returns 404 when `canvas-ui` flag not enabled

**Mock DAG Structure:**
```
Ritual (entry point)
  ├─→ Capsule (echo@1.0.0)
  │    └─→ Event Stream (demon.ritual.v1.events)
  │         └─→ NATS JetStream (infrastructure)
  ├─→ Approval Gate (deploy-gate)
  │    └─→ UI Endpoint (/api/approvals/grant)
  │         └─→ Event Stream (subscription)
  └─→ Policy Ward (security-policy)
```

### Future Integration

The Canvas UI is designed to integrate with:
- **`demonctl inspect --graph`** — CLI command for exporting graph data
- **NATS JetStream (SCALE_HINTS stream)** — Real-time telemetry feed
- **Server-Sent Events (SSE)** — Live updates at `/api/canvas/telemetry/stream`
- **Tenant/Run filtering** — Filter graph by `?tenant=<tenant>&run_id=<run_id>`

### Troubleshooting

**Canvas Page Returns 404**

```bash
# Check feature flag is set
echo $OPERATE_UI_FLAGS

# Enable Canvas UI
export OPERATE_UI_FLAGS=canvas-ui

# Restart Operate UI
cargo run -p operate-ui
```

**Navigation Link Not Visible**

Cause: Feature flag not set or `canvas_enabled` context variable not passed to templates

**Graph Not Rendering**

Possible causes:
- D3.js library failed to load
- Mock data structure invalid
- SVG rendering error

Check browser console for errors and verify D3 is loaded:
```javascript
console.log(d3.version); // Should print "7.9.0"
```

### Performance Considerations

For graphs exceeding 100 nodes:
- Force simulation may cause high CPU usage
- Consider static layouts or HTML5 Canvas/WebGL rendering
- Implement data pagination or filtering by run_id/tenant

### See Also

- [Canvas UI Documentation](../canvas-ui.md) — Comprehensive architecture and API integration guide
- [demonctl inspect](../cli-inspect.md) — CLI command for graph metrics inspection
- [Scale Feedback Telemetry](../scale-feedback.md) — Runtime telemetry schema and configuration

## In-UI Approval Actions

The UI now provides interactive approval controls for pending approval gates:

### Features
- **Approval buttons**: Grant/Deny buttons appear only for pending approvals on run detail pages
- **Real-time updates**: Approval status updates immediately via SSE when actions are taken
- **Input validation**: Email format validation and required reason for denials
- **User feedback**: Toast notifications for success, errors, and validation messages
- **Progressive enhancement**: Falls back gracefully if JavaScript is disabled

### Security & Authorization
- **Approver allowlist**: Set `APPROVER_ALLOWLIST=email1@company.com,email2@company.com` environment variable
- **CSRF protection**: Requires `X-Requested-With: XMLHttpRequest` header
- **Input validation**: Email format and required fields enforced both client and server-side
- **Audit trail**: All approval actions are logged and traceable via event timeline

### User Experience
1. Navigate to run detail page (`/runs/:runId`) with pending approval
2. Enter your email address in the approver field
3. Optionally add a note/reason in the text area
4. Click "Grant Approval" or "Deny Approval" button
5. See immediate feedback via toast notification
6. Watch approval status update in real-time
7. Approval actions section hides after resolution

### Error Handling
- **403 Forbidden**: User not in approver allowlist
- **409 Conflict**: Approval already resolved by another user
- **400 Bad Request**: Missing CSRF header or invalid input
- **Network errors**: Graceful handling with retry suggestions

## Approvals Endpoints — HTTP Semantics

Endpoints:
- `POST /api/approvals/:runId/:gateId/grant` body `{ approver, note? }`
- `POST /api/approvals/:runId/:gateId/deny` body `{ approver, reason }`

Behavior (first‑writer‑wins):
- First terminal for a gate → `200 OK` with JSON body of the published event.
- Duplicate terminal (same as current state) → `200 OK` with `{ "status": "noop" }` (no new event).
- Conflicting terminal (opposite of current state) → `409 CONFLICT` with `{ "error": "gate already resolved", "state": "granted|denied" }` (no new event).

Security Headers:
- `X-Requested-With: XMLHttpRequest` required for CSRF protection
- `Content-Type: application/json` required

Notes:
- Endpoints append events; they never mutate history. The run timeline is the source of truth.
- Idempotency keys: `approval.requested` uses `"<runId>:approval:<gateId>"`; terminals append `":granted"` or `":denied"`.
- Authorization checked against `APPROVER_ALLOWLIST` environment variable.

## Local Bootstrap & Troubleshooting
1) Start NATS
```bash
make dev   # exposes nats://127.0.0.1:4222
```

2) Clean env (no stream)
```bash
docker exec nats nats stream ls || true
```

3) Run UI (skip bootstrap to observe the banner)
```bash
DEMON_SKIP_STREAM_BOOTSTRAP=1 cargo run -p operate-ui
# GET /api/runs -> 200 {"runs":[]} with X-Demon-Warn
# Visit /runs -> shows "JetStream is not available. Unable to retrieve runs from the event store."
```

4) Create stream and publish two fixtures
```bash
export RITUAL_STREAM_NAME=RITUAL_EVENTS
export RITUAL_SUBJECTS="demon.ritual.v1.>"
python - <<'PY'
import asyncio, os, json
import nats
from nats.js.api import StreamConfig
async def main():
  nc = await nats.connect(os.getenv('NATS_URL','nats://127.0.0.1:4222'))
  js = nc.jetstream()
  try:
    await js.stream_info(os.getenv('RITUAL_STREAM_NAME','RITUAL_EVENTS'))
  except:
    await js.add_stream(StreamConfig(name=os.getenv('RITUAL_STREAM_NAME','RITUAL_EVENTS'), subjects=[os.getenv('RITUAL_SUBJECTS','demon.ritual.v1.>')]))
  subj='demon.ritual.v1.e2e-ritual.e2e-run.events'
  await js.publish(subj, json.dumps({"event":"ritual.started:v1","ritualId":"e2e-ritual","runId":"e2e-run","ts":"2025-01-01T00:00:00Z"}).encode(), headers={"Nats-Msg-Id":"e2e-run:1"})
  await js.publish(subj, json.dumps({
    "event":"ritual.completed:v1",
    "ritualId":"e2e-ritual",
    "runId":"e2e-run",
    "ts":"2025-01-01T00:00:05Z",
    "outputs":{
      "result":{
        "success":true,
        "data":{
          "echoed_message":"Hello from test",
          "character_count":15,
          "timestamp":"2025-01-01T00:00:05Z"
        }
      },
      "diagnostics":[{
        "level":"info",
        "message":"Echo operation completed",
        "timestamp":"2025-01-01T00:00:05Z"
      }],
      "metrics":{
        "counters":{"characterCount":15},
        "duration":{"total_ms":0.5}
      },
      "provenance":{
        "source":{"system":"echo-capsule","version":"0.0.1"},
        "timestamp":"2025-01-01T00:00:05Z"
      }
    }
  }).encode(), headers={"Nats-Msg-Id":"e2e-run:2"})
  await nc.drain()
asyncio.run(main())
PY
```

Manual seeding (one‑liners)
```bash
# Started
nats pub -H 'Nats-Msg-Id: e2e-run:1' \
  demon.ritual.v1.e2e-ritual.e2e-run.events \
  '{"event":"ritual.started:v1","ritualId":"e2e-ritual","runId":"e2e-run","ts":"2025-01-01T00:00:00Z"}'

# Completed
nats pub -H 'Nats-Msg-Id: e2e-run:2' \
  demon.ritual.v1.e2e-ritual.e2e-run.events \
  '{"event":"ritual.completed:v1","ritualId":"e2e-ritual","runId":"e2e-run","ts":"2025-01-01T00:00:05Z","outputs":{"result":{"success":true,"data":{"echoed_message":"Hello from test","character_count":15,"timestamp":"2025-01-01T00:00:05Z"}},"diagnostics":[{"level":"info","message":"Echo operation completed","timestamp":"2025-01-01T00:00:05Z"}],"metrics":{"counters":{"characterCount":15},"duration":{"total_ms":0.5}},"provenance":{"source":{"system":"echo-capsule","version":"0.0.1"},"timestamp":"2025-01-01T00:00:05Z"}}}'
```

5) Refresh UI
- /api/runs now lists the run
- /api/runs/e2e-run shows ordered events
- /runs and /runs/e2e-run render correctly
