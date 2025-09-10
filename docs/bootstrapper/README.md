# Bootstrapper (Self-Host v0)

`bootstrapper-demonctl` provides an idempotent one-command setup to ensure NATS JetStream stream/subjects, seed minimal events, and verify Operate UI readiness.

## Usage

```bash
# defaults: profile local-dev, run all phases (ensure + seed + verify)
cargo run -p bootstrapper-demonctl --

# explicit flags
cargo run -p bootstrapper-demonctl -- \
  --profile local-dev \
  --ensure-stream --seed --verify
```

## Env & Profiles
- `NATS_URL` or `NATS_HOST`/`NATS_PORT` (default `nats://127.0.0.1:4222`)
- `RITUAL_STREAM_NAME` precedence: env override → `RITUAL_EVENTS` → `DEMON_RITUAL_EVENTS`
- `RITUAL_SUBJECTS` (CSV, default `demon.ritual.v1.>`) | dedupe window: 120s
- `UI_URL` for verification (default `http://127.0.0.1:3000`)

## Verify criteria
- `/api/runs` returns an array with ≥1 element
- `/runs` returns HTML (basic template sanity)

## CI
A smoke step should start NATS + Operate UI + TTL worker, then run `bootstrapper-demonctl --ensure-stream --seed --verify` and assert exit 0.
