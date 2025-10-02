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
- `/api/runs`, `/api/runs/:runId` — JSON APIs (502 when NATS unavailable) (legacy - defaults to tenant "default")

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

### Future Enhancements
- **Real-time SSE**: Connect to graph commit streams for instant state updates (currently uses polling)
- **Approval Gates**: Display and interact with workflow approval gates
- **Policy Decisions**: Show policy evaluation results alongside task states
- **Runtime Integration**: Deep integration with runtime API for execution control

## Admin Probe (dev-only)

- Endpoint: `/admin/templates/report` returns JSON `{ template_ready, has_filter_tojson, templates }` used by the bootstrapper verify phase.
- Optional auth: set `ADMIN_TOKEN` in the environment to require header `X-Admin-Token: <token>`; without it, the probe is unauthenticated (dev-only).
- Admin: `/admin/templates/report` shows `template_ready=true` and `has_filter_tojson=true`.

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
