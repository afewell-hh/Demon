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

<<<<<<< Updated upstream
=======
### In-UI Approvals

Starting with Sprint 3, pending approvals can be managed directly from the Operate UI run detail page. When a run contains a pending approval gate, "Grant" and "Deny" buttons are automatically displayed.

**Environment Configuration:**
- `APPROVER_ALLOWLIST`: Comma-separated list of authorized approver emails (e.g., `ops@example.com,security@example.com`)

**Features:**
- Click "Grant" or "Deny" to take action on pending approvals
- Real-time updates via Server-Sent Events (SSE) — timeline updates within ~1 second
- First-writer-wins semantics with conflict detection (409 responses)
- Buttons automatically disable after successful action
- Clear error messages for authorization failures

**Example:**
1. Start the UI: `APPROVER_ALLOWLIST="ops@example.com" cargo run -p operate-ui`
2. Navigate to a run with pending approval: `/runs/{run_id}`
3. Click "Grant" or "Deny" and provide your email when prompted

## Tenancy

Demon supports multi-tenant namespace isolation for events and resources. When enabled, events are published to tenant-scoped subjects and isolated from other tenants.

### Configuration

**Environment Variables:**
- `TENANTING_ENABLED=0|1` — Enable/disable tenant isolation (default: 0)
- `TENANT_DEFAULT=default` — Default tenant when none specified (default: "default")
- `TENANT_ALLOWLIST=default,acme,globex` — Comma-separated list of allowed tenants (optional)
- `TENANT_DUAL_PUBLISH=0|1` — Publish to both tenant and legacy subjects during migration (default: 0)

### Usage

**Headers and Query Parameters:**
- HTTP Header: `X-Demon-Tenant: tenant-name`
- Query Parameter: `?tenant=tenant-name`

**Event Subject Schema:**
- **Enabled**: `demon.ritual.v1.<tenant>.<ritualId>.<runId>.events`
- **Disabled**: `demon.ritual.v1.<ritualId>.<runId>.events` (legacy)

### Examples

```bash
# Query runs for specific tenant
curl -H 'X-Demon-Tenant: acme' 'http://127.0.0.1:3000/api/runs?limit=5'
curl 'http://127.0.0.1:3000/api/runs?tenant=acme&limit=5'

# Get run details for specific tenant
curl 'http://127.0.0.1:3000/api/runs/<run_id>?tenant=acme'

# Subscribe to tenant-scoped event stream
curl 'http://127.0.0.1:3000/api/runs/<run_id>/events/stream?tenant=acme'

# Run ritual with tenanting enabled
TENANTING_ENABLED=1 TENANT_DEFAULT=acme make dev
```

**Migration Support:**
Enable `TENANT_DUAL_PUBLISH=1` to publish events to both tenant-scoped and legacy subjects during transition periods.

### Per‑Tenant Quotas

When `TENANTING_ENABLED=1`, quota counters are keyed per tenant and capability; when disabled they are global by capability.

Environment:
- `WARDS_CAP_QUOTAS` — supports JSON and compact formats
  - JSON (tenant → capability → quota):
    `{ "acme": { "capsule.echo": { "limit": 2, "windowSeconds": 60 } } }`
  - Compact (global and tenant overrides):
    `GLOBAL:capsule.echo=5:60,TENANT:acme:capsule.echo=2:60`
- `WARDS_QUOTAS` — per‑tenant fallback: `{ "acme": { "limit": 100, "windowSeconds": 300 } }`
- `WARDS_GLOBAL_QUOTA` — global fallback for all caps/tenants: `{ "limit": 1000, "windowSeconds": 600 }`

Behavior:
- With tenancy enabled, counters key as `<tenant>:<capability>`; otherwise `<capability>`.
- Effective quota precedence: tenant‑cap override → tenant fallback → global‑cap fallback → global fallback.

Example (compact format):
```
TENANTING_ENABLED=1 TENANT_DEFAULT=default \
WARDS_CAP_QUOTAS='GLOBAL:capsule.echo=5:60,TENANT:acme:capsule.echo=2:60' \
make dev
```
Drive runs for both `default` and `acme`; `capsule.echo` remaining counts decrease independently, with `acme` limited to 2 per 60s and others to 5.

>>>>>>> Stashed changes
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
