![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)
> Preview Kit: see docs/preview/alpha/README.md

- [Preview Kit](docs/preview/alpha/README.md)
- [Bundle Library & Signatures](docs/bootstrapper/bundles.md) (offline, reproducible, CI-enforced)
- [Contract Bundle Releases](docs/contracts/releases.md) (automated GitHub Releases for contract schemas)

<sub>Local verify:</sub>
<code>target/debug/demonctl bootstrap --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
| jq -e 'select(.phase=="verify" and .signature=="ok")' >/dev/null && echo "signature ok"</code>

## Self-Host Bootstrap

Use the `demonctl bootstrap` command for zero-config self-hosting setup:

```bash
# Complete bootstrap (ensure stream + seed events + verify UI)
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify

# Individual steps
cargo run -p demonctl -- bootstrap --ensure-stream    # Create NATS stream
cargo run -p demonctl -- bootstrap --seed            # Seed sample events
cargo run -p demonctl -- bootstrap --verify          # Verify Operate UI health

# With environment overrides
RITUAL_STREAM_NAME=CUSTOM_STREAM cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify
```

See [docs/bootstrapper/README.md](docs/bootstrapper/README.md) for more details.


# Demon — Meta-PaaS (Milestone 0)

[![Preview: Alpha](https://img.shields.io/badge/Preview-Alpha-6f42c1.svg)](https://github.com/afewell-hh/Demon/releases/tag/preview-alpha-1)

Thin-slice bootstrapping of the Demon project.

## Quickstart

```bash
make dev            # bring up NATS JetStream & build workspace
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

Expected output:

The echo capsule prints `Hello from Demon!`

A JSON event for `ritual.completed:v1` is printed to stdout.

**Note**: M0-3 includes per-capability quotas with policy decisions. Default quotas allow reasonable development usage without configuration.

### Contract Registry

Export all contracts (schemas and WIT definitions):

```bash
# Summary view
cargo run -p demonctl -- contracts bundle

# Include WIT definitions
cargo run -p demonctl -- contracts bundle --include-wit

# Export as JSON
cargo run -p demonctl -- contracts bundle --format json --include-wit
```

#### Publishing Contract Bundles

For distribution and automation, use the bundling Make target:

```bash
# Create distributable contract bundle
make bundle-contracts
```

This creates:
- `dist/contracts/bundle.json` - Complete contract bundle with schemas and WIT definitions
- `dist/contracts/manifest.json` - Metadata including version, timestamp, git info, and SHA-256 digest

The bundle is automatically generated and uploaded as CI artifacts on main branch merges. Each bundle includes a SHA-256 digest for integrity verification. External teams can download contract artifacts from GitHub Actions artifacts for integration.

#### Fetching Contract Bundles

Download the latest contract bundle from GitHub Actions artifacts:

```bash
# Fetch latest bundle to default location (contracts/bundle.json)
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle

# Fetch to custom location with manifest
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle \
  --dest downloaded/bundle.json --manifest

# Fetch from specific repository
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle \
  --repo owner/repo --dest bundle.json

# Fetch specific workflow run
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle \
  --run-id 12345 --dest bundle.json

# Skip verification (NOT RECOMMENDED)
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle \
  --skip-verification --dest bundle.json
```

**Security**: By default, all downloaded bundles are verified using SHA-256 digests from the manifest. This ensures bundle integrity and authenticity.

**Manual Verification**: You can verify a bundle manually using:
```bash
# Compare with manifest
EXPECTED=$(jq -r '.bundle_sha256' manifest.json)
ACTUAL=$(shasum -a 256 bundle.json | cut -d' ' -f1)
echo "Expected: $EXPECTED"
echo "Actual:   $ACTUAL"
```

**Requirements**: GitHub token with `actions:read` scope set as `GH_TOKEN` or `GITHUB_TOKEN` environment variable.

## Approvals API

The M0-4 Approvals API provides REST endpoints for granting and denying approval gates:

```bash
# Grant approval (first-writer-wins)
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/grant \
  -H "Content-Type: application/json" \
  -d '{"approver": "ops@example.com", "note": "approved for production"}'

# Deny approval
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/deny \
  -H "Content-Type: application/json" \
  -d '{"approver": "ops@example.com", "reason": "security review required"}'
```

**Behavior**: First terminal decision wins (200 OK), subsequent conflicts return 409. Duplicate decisions return 200 with noop status.

## Layout

- `engine/` — minimal ritual interpreter (M0).
- `runtime/` — link-name router (stubs).
- `capsules/echo/` — hello capsule.
- `contracts/` — JSON Schemas + future WIT.
- `demonctl/` — CLI to run rituals.
- `docker/dev` — NATS JetStream profile.

## Next

- Wire the event to NATS (JetStream) instead of stdout (M1).
- Add durable timers & replays.
- Add Operate UI (read-only).

<!-- audit-kick -->

## Project Process

- One‑pager: docs/process/MVP.md
- Branch protections (MVP): docs/process/branch_protection_mvp.md
- Project board: https://github.com/users/afewell-hh/projects/1

# Trigger CI re-run
