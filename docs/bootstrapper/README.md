# Bootstrapper (Self-Host v0)

The bootstrapper provides an idempotent one-command setup to ensure NATS JetStream stream/subjects, seed minimal events, and verify Operate UI readiness.

## Main CLI Usage (Recommended)

Use the main `demonctl bootstrap` subcommand:

```bash
# Complete bootstrap (all phases: ensure + seed + verify)
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify

# Individual steps
cargo run -p demonctl -- bootstrap --ensure-stream    # Create NATS stream only
cargo run -p demonctl -- bootstrap --seed            # Seed sample events only
cargo run -p demonctl -- bootstrap --verify          # Verify Operate UI health only

# With overrides
cargo run -p demonctl -- bootstrap \
  --ensure-stream --seed --verify \
  --nats-url nats://127.0.0.1:4222 \
  --stream-name CUSTOM_STREAM \
  --ui-base-url http://127.0.0.1:3000
```

## Direct bootstrapper-demonctl Usage

For advanced use cases, you can use the standalone tool:

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
- `/admin/templates/report` JSON:
  - `template_ready: true` — templates compiled and ready.
  - `has_filter_tojson: true` — JSON filter is available for templates.
- `/api/runs` returns an array with ≥1 element.

## Stream precedence

Bootstrapper resolves the stream name with precedence:

1. `RITUAL_STREAM_NAME` (recommended)
2. `DEMON_RITUAL_EVENTS` (deprecated; logs a deprecation warning)
3. `RITUAL_EVENTS` (default)

## CI
A smoke step should start NATS + Operate UI + TTL worker with `APPROVER_ALLOWLIST=ops@example.com`, then run `bootstrapper-demonctl --ensure-stream --seed --verify` twice and assert exit 0.
