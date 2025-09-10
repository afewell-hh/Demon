# Preview Alpha — Dry‑Run Checklist (Clean VM)

Tag & SHA
- preview-alpha-1 (27e36b21136e)

Prereqs
- OS: Ubuntu 22.04 (or macOS 13+)
- Docker, Rust 1.82.0, jq, wkhtmltoimage (optional for screenshots)

Steps (≈10 minutes)
- Clone and checkout tag
  - `git clone https://github.com/afewell-hh/Demon.git && cd Demon`
  - `git fetch --tags && git checkout preview-alpha-1`
- Start NATS JetStream
  - `make dev` (or set `NATS_PORT=4222` override if needed)
- Start Operate UI + TTL worker
  - `RITUAL_STREAM_NAME=RITUAL_EVENTS APPROVER_ALLOWLIST=ops@example.com cargo run -p operate-ui &`
  - `TTL_WORKER_ENABLED=1 RITUAL_STREAM_NAME=RITUAL_EVENTS cargo run -p engine --bin demon-ttl-worker &`
- Seed preview runs (idempotent)
  - `./examples/seed/seed_preview.sh`
- Verify API
  - `/api/runs` → `jq 'length >= 1'`
  - Granted: `run-preview-b` has `approval.granted:v1`
  - TTL: `run-preview-c` has one `approval.denied:v1` with `reason:"expired"`
- Verify HTML
  - `/runs` and `/runs/<id>` render (no template errors)
- Optional: capture screenshots
  - runs_list.png → `/runs`
  - approval_granted.png → `/runs/run-preview-b`
  - ttl_expired.png → `/runs/run-preview-c`

Troubleshooting
- Port clash: set `NATS_PORT` and/or `NATS_URL`; rerun services and the seeder.
- Empty list: re-run the seeder; it’s idempotent via `Nats-Msg-Id`.
- Grant fails: ensure `APPROVER_ALLOWLIST=ops@example.com` on the UI process.
