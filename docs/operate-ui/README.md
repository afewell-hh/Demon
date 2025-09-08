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
