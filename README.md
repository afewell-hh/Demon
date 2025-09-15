![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)
> Preview Kit: see docs/preview/alpha/README.md

- [Preview Kit](docs/preview/alpha/README.md)
- [Bundle Library & Signatures](docs/bootstrapper/bundles.md) (offline, reproducible, CI-enforced)

<sub>Local verify:</sub>  
<code>target/debug/demonctl bootstrap --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
| jq -e 'select(.phase=="verify" and .signature=="ok")' >/dev/null && echo "signature ok"</code>


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

