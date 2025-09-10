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
- `/runs` — recent runs, stable ordering
- `/runs/:runId` — ordered timeline per run
- `/api/runs`, `/api/runs/:runId` — JSON APIs (502 when NATS unavailable)

## Notes
- Read-only semantics: ephemeral consumers; no durable state created by the UI.
- Deterministic fetch: multi-batch reads until a short batch; no hangs.
- Failure mode: if NATS is down, HTML pages render a friendly error; APIs return 502.
- Review protocol: open PR as Draft, satisfy the Evidence Checklist, then freeze at a commit SHA for review.
- Stream selection: set `RITUAL_STREAM_NAME` (default `RITUAL_EVENTS`). If absent, the UI will fall back to the legacy `DEMON_RITUAL_EVENTS` stream and log a deprecation warning.

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
 - Preview Mode links:
   - Runbook (One‑Pager): `docs/preview/alpha/runbook.md`
   - Client Deck (5 slides): `docs/preview/alpha/deck.md`
   - Presenter Script (60‑sec): `docs/preview/alpha/presenter_script.md`
   - Dry‑Run Checklist: `docs/preview/alpha/dry_run_checklist.md`
- Admin: `/admin/templates/report` shows `template_ready=true` and `has_filter_tojson=true`.

## Approvals Endpoints — HTTP Semantics

Endpoints:
- `POST /api/approvals/:runId/:gateId/grant` body `{ approver, note? }`
- `POST /api/approvals/:runId/:gateId/deny` body `{ approver, reason }`

Behavior (first‑writer‑wins):
- First terminal for a gate → `200 OK` with JSON body of the published event.
- Duplicate terminal (same as current state) → `200 OK` with `{ "status": "noop" }` (no new event).
- Conflicting terminal (opposite of current state) → `409 CONFLICT` with `{ "error": "gate already resolved", "state": "granted|denied" }` (no new event).

Notes:
- Endpoints append events; they never mutate history. The run timeline is the source of truth.
- Idempotency keys: `approval.requested` uses `"<runId>:approval:<gateId>"`; terminals append `":granted"` or `":denied"`.
