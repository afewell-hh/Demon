# Bootstrapper (Self-Host v0)

The bootstrapper provides an idempotent one-command setup to ensure NATS JetStream stream/subjects, seed minimal events, and verify Operate UI readiness.

## Main CLI Usage (Recommended)

Use the main `demonctl bootstrap` subcommand:

```bash
# Complete bootstrap using profile defaults (all phases: ensure + seed + verify)
cargo run -p demonctl -- bootstrap --profile local-dev --ensure-stream --seed --verify
cargo run -p demonctl -- bootstrap --profile remote-nats --ensure-stream --seed --verify

# Individual steps
cargo run -p demonctl -- bootstrap --ensure-stream    # Create NATS stream only
cargo run -p demonctl -- bootstrap --seed            # Seed sample events only
cargo run -p demonctl -- bootstrap --verify          # Verify Operate UI health only

# With explicit bundle (overrides profile defaults)
cargo run -p demonctl -- bootstrap \
  --bundle examples/bundles/local-dev.yaml \
  --ensure-stream --seed --verify

# With library bundle (local provider)
cargo run -p demonctl -- bootstrap \
  --bundle lib://local/preview-local-dev@0.0.1 \
  --ensure-stream --seed --verify

# With remote bundle (HTTPS provider - requires index with baseUrl)
cargo run -p demonctl -- bootstrap \
  --bundle lib://https/remote-bundle@1.0.0 \
  --ensure-stream --seed --verify

# Verify-only mode (checks bundle integrity without NATS/UI)
cargo run -p demonctl -- bootstrap \
  --bundle lib://local/preview-local-dev@0.0.1 \
  --verify-only

cargo run -p demonctl -- bootstrap \
  --bundle lib://https/remote-bundle@1.0.0 \
  --verify-only

# With command line overrides (precedence: flags > bundle > env)
cargo run -p demonctl -- bootstrap \
  --profile local-dev \
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

## Bundle Library and Remote Registry

The bootstrapper supports fetching bundles from both local and remote sources using URIs:

**URI Formats:**
- `lib://local/{name}@{version}` - Resolves from local index (`bootstrapper/library/index.json`)
- `lib://https/{name}@{version}` - Fetches from remote HTTPS registry

**Remote Bundle Features:**
- Downloads are cached in temp directory for the session
- Canonical digest verification ensures integrity
- Signature verification uses the same Ed25519 flow as local bundles
- HTTP errors and digest mismatches fail immediately

**Index Configuration:**
For HTTPS provider, the index must specify:
```json
{
  "provider": "https",
  "baseUrl": "https://registry.example.com",
  "bundles": [...]
}
```

## Profiles & Bundle Resolution

demonctl supports profile-based configuration with automatic bundle resolution:

**Profiles:**
- `local-dev` (default) → `examples/bundles/local-dev.yaml`
- `remote-nats` → `examples/bundles/remote-nats.yaml`

**Configuration Precedence:**
1. Command line flags (highest priority)
2. Bundle configuration (if specified via `--bundle` or profile default)
3. Environment variables (lowest priority)

**Environment Variables:**
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
A smoke step should start NATS + Operate UI + TTL worker with `APPROVER_ALLOWLIST=ops@example.com`, then run `demonctl bootstrap --profile local-dev --ensure-stream --seed --verify` twice and assert exit 0.
