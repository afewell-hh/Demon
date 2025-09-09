# Customer Preview (Alpha) — Runbook

This 10‑minute preview shows Demon runs, policy decisions, approvals grant/deny, and TTL auto‑deny.

Prereqs
- Linux/macOS dev box; Docker; Rust 1.82.0; jq.

Environment
- `NATS_URL` (default `nats://127.0.0.1:4222`)
- `RITUAL_STREAM_NAME` precedence: env override → `RITUAL_EVENTS` → `DEMON_RITUAL_EVENTS`
- `WARDS_ENABLED=1` with example caps/quotas (see below)
- `APPROVER_ALLOWLIST="ops@example.com"`
- `APPROVAL_TTL_SECONDS=5`
- `TTL_WORKER_ENABLED=1`

Start services
```bash
make dev                                 # starts NATS JetStream (4222/8222)
RITUAL_STREAM_NAME=RITUAL_EVENTS \
  cargo run -p operate-ui &              # UI at http://127.0.0.1:3000
TTL_WORKER_ENABLED=1 \
  cargo run -p engine --bin demon-ttl-worker &
```

Seed demo runs (idempotent)
```bash
./examples/seed/seed_preview.sh
```
Expected output ends with three subjects and run IDs.

Verify via API
```bash
curl -s http://127.0.0.1:3000/api/runs | jq 'length >= 1'
RUN_B=run-preview-b
RUN_C=run-preview-c
curl -s http://127.0.0.1:3000/api/runs/$RUN_B | jq '.events | map(.event) | index("approval.granted:v1") != null'
curl -s http://127.0.0.1:3000/api/runs/$RUN_C | jq '.events | map(select(.event=="approval.denied:v1" and .reason=="expired")) | length == 1'
```

UI walkthrough
- /runs — lists seeded runs
- /runs/<runId> — shows events; look for Policy Decision (ALLOW/DENY), Approvals: Granted, and “Denied — expired”
- /admin/templates/report — template report; `template_ready=true`, `has_filter_tojson=true`

Wards example config (optional)
```bash
export WARDS_ENABLED=1
export WARDS_CAPS='{"capsule.http":true,"capsule.echo":true}'
export WARDS_CAP_QUOTAS='{"capsule.http":"1/min","capsule.echo":"5/min"}'
```

Troubleshooting
- Port clash: set `NATS_PORT` and `NATS_URL` accordingly; re‑run `make dev`.
- Empty UI: ensure stream exists; seed script creates `RITUAL_EVENTS` with `demon.ritual.v1.>` subject.
