# Preview Day — One‑Pager (Alpha)

Project: Demon — Meta‑PaaS to build vPaaS platforms

Build SHA (preview-alpha-1 tag): 27e36b21136e

Goal
- Give the client a deterministic, 10‑minute preview of Demon’s core capabilities on a clean machine. They should be able to reproduce the demo end‑to‑end using the included runbook and seeder.

What We’ll Show
- Runs list & detail in Operate UI (HTML + JSON API).
- Policy Decisions (Wards) — per‑capability quotas: allow → deny, with `policy.decision:v1` events and camelCase quota block.
- Approvals Gate — request → grant/deny with first‑writer‑wins and idempotency.
- TTL Auto‑Deny — pending approval expires; TTL worker emits `approval.denied:v1` (reason:"expired").

Prereqs
- Linux/macOS dev box; Docker; Rust 1.82.0; jq.

Environment (Preview Defaults)
- `RITUAL_STREAM_NAME=RITUAL_EVENTS` (fallback to deprecated `DEMON_RITUAL_EVENTS` handled)
- `WARDS_ENABLED=1`
- `WARDS_CAPS='{"tenant-a":["capsule.http","capsule.echo"]}'`
- `WARDS_CAP_QUOTAS='{"tenant-a":{"capsule.http":{"limit":1,"windowSeconds":60},"capsule.echo":{"limit":5,"windowSeconds":60}}}'`
- `APPROVER_ALLOWLIST="ops@example.com"`
- `APPROVAL_TTL_SECONDS=5`
- `TTL_WORKER_ENABLED=1`
- Optional: `NATS_PORT=4222` (override if port clash)

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

Flow of the Demo (10 minutes)
1) Start services (NATS, Operate UI, TTL worker). Confirm `/admin/templates/report` shows `template_ready:true` and `has_filter_tojson:true`.
2) Seed three runs using `examples/seed/seed_preview.sh`.
3) Show UI: `/runs` lists runs; open each run detail to show timeline.
4) Prove durability: refresh detail pages; show events persisted in JetStream.
5) Show approvals REST path: explain first‑writer‑wins (200 OK noop vs 409 conflict) — already exercised by seeder.

Verify via API
```bash
curl -s http://127.0.0.1:3000/api/runs | jq 'length >= 1'
RUN_B=run-preview-b
RUN_C=run-preview-c
curl -s http://127.0.0.1:3000/api/runs/$RUN_B | jq '.events | map(.event) | index("approval.granted:v1") != null'
curl -s http://127.0.0.1:3000/api/runs/$RUN_C | jq '.events | map(select(.event=="approval.denied:v1" and .reason=="expired")) | length == 1'
```

Success Criteria
- `/api/runs` returns an array with ≥1 run.
- Run A includes `policy.decision:v1` with `reason:null` then `reason:"limit_exceeded"`.
- Run B includes `approval.requested:v1` then `approval.granted:v1`.
- Run C includes one `approval.denied:v1` with `reason:"expired"`.
- Operate UI pages render without template errors.
- `nats stream info $RITUAL_STREAM_NAME` shows subjects `demon.ritual.v1.>` and a non‑zero message count.

Talking Points
- Interface‑first: contracts (schemas) + golden fixtures drive UI/API.
- Determinism: idempotent message keys; durable timers; replayable event log.
- Policy Everywhere: deny‑by‑default, quotas, approvals, TTL.
- Cloud‑agnostic: wasmCloud/NATS first; Kubernetes optional later.

FAQ
- Q: What if NATS port is busy? A: Set `NATS_PORT` and re‑run the seeder; CI and scripts honor it.
- Q: Why camelCase in events? A: Templates & APIs consume stable camelCase VMs/JSON for frontend consistency.
- Q: Multi‑node counters? A: Per‑process counters noted in ADR‑0004; acceptable for Alpha; cross‑node strategy is a later ADR.

Appendix — Verification Snippets
```
curl -s http://127.0.0.1:3000/api/runs | jq 'length >= 1'
curl -s http://127.0.0.1:3000/api/runs/<grantRun> | jq '.events | map(.event) | index("approval.granted:v1") != null'
curl -s http://127.0.0.1:3000/api/runs/<ttlRun> | jq '.events | map(select(.event=="approval.denied:v1" and .reason=="expired")) | length == 1'
```

Presenter & Dry‑Run
- Presenter Script (60‑sec): `docs/preview/alpha/presenter_script.md`
- Dry‑Run Checklist: `docs/preview/alpha/dry_run_checklist.md`
