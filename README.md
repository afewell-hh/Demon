# Demon

[![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)](https://github.com/afewell-hh/demon/actions/workflows/ci.yml)
[![Preview: Alpha](https://img.shields.io/badge/Preview-Alpha-6f42c1.svg)](https://github.com/afewell-hh/demon/releases/tag/preview-alpha-1)

**Secure, auditable workflow automation built for regulated delivery teams.**

## Why Demon

Modern platform and security groups are stuck between brittle CI/CD pipelines and manual runbooks that quietly drift. Demon exists to let those teams script complex operations with the same rigor they bring to code: declared once, replayed safely, and inspected afterward. A single ritual capsule can roll out a release, rotate credentials, or steer an incident bridge, yet every action is still gated by explicit policy and human approval where needed. Instead of duct-taping bespoke bots, Demon provides a dependable control plane with transparent audit trails that operations, compliance, and engineering can trust.

### Track C: Contracts, Envelopes & Replay

Every workflow step emits a **signed envelope** that captures inputs, outputs, and policy context. The runtime persists those envelopes in NATS JetStream so they can be replayed deterministically, compared across environments, and verified offline. **Idempotency** is enforced by re-hydrating envelopes before executing a capsule; if the payload matches, Demon proves nothing new happened and skips the duplicate. This lets teams retry failed automation, recover from transient outages, and build "approve once, run many" rituals without fear of double execution.

- **Contracts** (JSON Schemas + WIT) govern capsule interfaces and event shapes, ensuring runtime compatibility across dev, staging, and prod.
- **Envelopes** wrap each event with metadata (timestamp, nonce, signature), making every step portable, auditable, and reproducible.
- **Replay guarantees** allow operations teams to re-run rituals from checkpoint or re-verify completed workflows without side effects, powered by fingerprint matching and deterministic execution.

The contract registry keeps schema changes honest, while approval gates ensure humans stay in the loop exactly where their judgment matters.

## Agent-first Automation

Demon is designed for automated workflows that must survive restarts, handle retries gracefully, and provide audit trails that satisfy compliance requirements. Capsules are agent-like processes that respond to ritual steps, emit events, and declare their capabilities via contracts.

- **Contracts stay ahead of code**: Capsule workflows negotiate via versioned JSON Schemas and WIT definitions stored in `contracts/`, keeping runtime compatibility across environments. Schema validation runs in CI (`contracts-validate` required check) to catch breaking changes before they reach production.
- **Envelopes make state portable**: Every capsule call, policy decision, and emitted event produces an envelope stored on JetStream, ready for replay, audit, or downstream automation. Each envelope includes a cryptographic signature for offline verification.
- **Idempotent replays by default**: Rituals hydrate prior envelopes, compare their fingerprints, and only re-execute when inputs change—perfect for "retry without side effects" approvals. This lets operators re-run failed workflows or recover from transient infrastructure issues without double-applying changes.
- **Guardrails are built in**: Policy wards enforce quotas, time windows, and escalation paths, while approval gates provide human checkpoints with first-writer-wins semantics. All policy decisions emit `policy.decision:v1` events that become part of the audit log.

**Expectations for capsule authors**: Capsules must be idempotent, emit deterministic events given the same envelope inputs, and respect contract schemas. Non-compliant capsules will fail schema validation or produce replay drift.

## Governance Guardrails

Demon enforces process discipline through required CI checks, code review hygiene, and cryptographic provenance for all contract bundles. These guardrails protect `main` from untested changes, ensure every review comment receives a response, and guarantee that promoted artifacts match their signed manifests.

### Required Checks (DO NOT RENAME)

Five status checks must pass before any PR merges to `main`. Job names are protected and **must** match exactly:

1. **`Bootstrapper bundles — verify (offline, signature ok)`** — Confirms that signed contract bundles can be verified offline without network access.
2. **`Bootstrapper bundles — negative verify (tamper ⇒ failed)`** — Tests that tampered bundles fail verification, proving signature integrity.
3. **`contracts-validate`** — Runs `scripts/contracts-validate.sh` to ensure JSON Schemas, fixtures, and WIT definitions are internally consistent and backward-compatible.
4. **`review-lock-guard`** — Enforces that the PR body contains `Review-lock: <sha>` matching the current HEAD, preventing stale approvals from merging changed code.
5. **`review-threads-guard (PR) / guard`** — Verifies that every review comment has received an explicit author reply before merge.

If you rename a job or context in `.github/workflows/ci.yml`, branch protection will fail and PRs will be blocked. Always update protection snapshots (`.github/snapshots/branch-protection-YYYY-MM-DD.json`) when changing CI structure.

### Review-lock Discipline

Every pull request body **must** include:

```
Review-lock: <40-character-sha>
```

This SHA must match the current HEAD of the PR branch. Update it on every push:

```bash
PR=123
HEAD=$(gh pr view $PR --json headRefOid -q .headRefOid)
gh pr edit $PR -b "$(gh pr view $PR -q .body)\n\nReview-lock: $HEAD"
```

Review-lock prevents approvals from carrying forward when new commits land, ensuring reviewers always see the final code that will merge.

### Replies Policy

**Every review comment must receive an explicit author reply before merge.** Use one of:

- `"Fixed in <short-sha>"`
- `"Clarified: …"`
- `"Won't fix because …"`

Conversation resolution is enabled on `main`; resolve threads **after** replying. Docs-only PRs are exempt from `review-threads-guard` (the guard self-skips), but substantive feedback still deserves a reply.

### Provenance Verification

Contract bundles are signed at build time and verified before deployment:

```bash
# Local verification
cargo run -p demonctl -- bootstrap --verify-only

# CI enforcement
scripts/contracts-validate.sh
```

Tampered bundles fail verification; only artifacts with valid signatures can be promoted to staging or production. See [docs/bootstrapper/bundles.md](docs/bootstrapper/bundles.md) for offline verification workflows.

## Quickstart

```bash
# Start the development environment (NATS + workspace build)
make dev

# Run the sample echo ritual
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

You will see envelopes emitted for `ritual.started` and `ritual.completed`; approvals can be injected via Operate UI or the REST API.

## Self-host Bootstrap

```bash
# End-to-end bootstrap: stream, seed data, UI verification
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify

# Targeted steps
cargo run -p demonctl -- bootstrap --ensure-stream
cargo run -p demonctl -- bootstrap --seed
cargo run -p demonctl -- bootstrap --verify

# Override defaults when promoting to higher environments
RITUAL_STREAM_NAME=PROD_STREAM cargo run -p demonctl -- bootstrap --ensure-stream
```

Production deployment patterns and offline verification live in [docs/bootstrapper/README.md](docs/bootstrapper/README.md).

## Contract Registry

```bash
# Fetch the latest bundle (requires GitHub token)
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle

# Export contracts for integration testing
cargo run -p demonctl -- contracts bundle --format json --include-wit
```

Schemas, fixtures, and WIT interfaces live under [`contracts/`](contracts/); update goldens whenever events change so automated compatibility checks stay reliable.

## Approvals API

```bash
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/grant \
  -H "Content-Type: application/json" \
  -H "X-Demon-API-Version: v1" \
  -d '{"approver": "ops@example.com", "note": "approved for production"}'
```

First-writer-wins semantics and TTL auto-deny keep approvals deterministic. See [docs/operate-ui/README.md](docs/operate-ui/README.md) for UI workflows and REST responses.

## API Versioning

All API endpoints under `/api/` support version negotiation via the `X-Demon-API-Version` header:

```bash
# Explicitly request v1 API
curl -H "X-Demon-API-Version: v1" http://localhost:3000/api/runs

# Without version header (defaults to v1 for backwards compatibility)
curl http://localhost:3000/api/runs
```

**Server behavior:**
- All API responses include `X-Demon-API-Version: v1` header
- Unsupported versions return `406 Not Acceptable` with error details
- Current stable version: **v1**

**Versioned APIs:**
- Ritual Runs API (`/api/runs`, `/api/runs/:run_id`)
- Approval Management API (`/api/approvals/*`)
- Contract Registry API (`/api/contracts/*`)

For complete versioning policy, contract specifications, and client integration guidance, see [docs/api-versioning.md](docs/api-versioning.md).

## Layout

- **`engine/`** — Ritual interpreter that executes workflows and emits envelopes
- **`runtime/`** — Capsule runtime with link-name routing and sandboxing
- **`demonctl/`** — CLI for rituals, contract management, and bootstrapping
- **`operate-ui/`** — Read-only dashboard for monitoring runs and approvals
- **`contracts/`** — JSON Schemas, fixtures, and WIT definitions that govern automation
- **`capsules/echo/`** — Reference capsule showcasing the runtime contract

## Docker Build & Publish

The alpha Docker pipeline publishes `engine`, `runtime`, and `operate-ui` images to GHCR with `latest`, branch, and `sha-<commit>` tags.

- Local verification: run `make build`, then build container images (`docker build -f <component>/Dockerfile ...`) as detailed in [docs/how-to-guides/docker-pipeline.md](docs/how-to-guides/docker-pipeline.md).
- CI workflow: [`.github/workflows/docker-build.yml`](.github/workflows/docker-build.yml) builds on pull requests and publishes on merges; caching shortens retries.
- **Digest fetching**: Operators can fetch immutable `sha256:...` digests via `demonctl docker digests fetch` for reproducible deployments — see [K8s Bootstrap README](docs/examples/k8s-bootstrap/README.md#fetch-ghcr-digests-outside-ci) for usage.
- Troubleshooting: [docs/ops/docker-troubleshooting.md](docs/ops/docker-troubleshooting.md) covers profiling, retries, and multi-arch considerations.

## Community & Roadmap

- 📋 [Roadmap and milestones](https://github.com/users/afewell-hh/projects/1)
- 🐛 [Issue tracker](https://github.com/afewell-hh/demon/issues)
- 📖 [Documentation hub](docs/)
- 💬 [Project discussions](https://github.com/afewell-hh/demon/discussions)

## Project Process

- **MVP Contract**: [docs/mvp/01-mvp-contract.md](docs/mvp/01-mvp-contract.md) captures personas, M0 must-haves, and acceptance criteria.
- **Branch Protection**: [docs/process/branch_protection_mvp.md](docs/process/branch_protection_mvp.md) details required checks, review-lock cadence, and replies guard expectations.
- **PM Playbook**: [docs/process/PM_REBOOT_PLAYBOOK.md](docs/process/PM_REBOOT_PLAYBOOK.md) keeps Track C terminology aligned across docs and rituals.

## Preview Resources

- 🔧 [Alpha Preview Kit](docs/preview/alpha/README.md) — hands-on evaluation checklist.
- 📦 [Bundle Library & Signatures](docs/bootstrapper/bundles.md) — offline verification and CI enforcement.
- 📋 [Contract Bundle Releases](docs/contracts/releases.md) — automated schema distribution.

<sub>**Local verification snippet**</sub>

```bash
target/debug/demonctl bootstrap --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
| jq -e 'select(.phase=="verify" and .signature=="ok")' >/dev/null && echo "signature ok"
```
