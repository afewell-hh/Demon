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
# Visit /runs -> shows "No event stream found. See Runbook: setup."
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
  await js.publish(subj, json.dumps({"event":"ritual.completed:v1","ritualId":"e2e-ritual","runId":"e2e-run","ts":"2025-01-01T00:00:05Z"}).encode(), headers={"Nats-Msg-Id":"e2e-run:2"})
  await nc.drain()
asyncio.run(main())
PY
```

5) Refresh UI
- /api/runs now lists the run
- /api/runs/e2e-run shows ordered events
- /runs and /runs/e2e-run render correctly
