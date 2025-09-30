# Demon

[![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)](https://github.com/afewell-hh/demon/actions/workflows/ci.yml)
[![Preview: Alpha](https://img.shields.io/badge/Preview-Alpha-6f42c1.svg)](https://github.com/afewell-hh/demon/releases/tag/preview-alpha-1)

**Secure, auditable workflow automation built for regulated delivery teams.**

## Why Demon

Modern platform and security groups are stuck between brittle CI/CD pipelines and manual runbooks that quietly drift. Demon exists to let those teams script complex operations with the same rigor they bring to code: declared once, replayed safely, and inspected afterward. A single ritual capsule can roll out a release, rotate credentials, or steer an incident bridge, yet every action is still gated by explicit policy and human approval where needed. Instead of duct-taping bespoke bots, Demon provides a dependable control plane with transparent audit trails that operations, compliance, and engineering can trust.

Every workflow step emits a signed envelope that captures inputs, outputs, and policy context. The runtime persists those envelopes in NATS JetStream so they can be replayed deterministically, compared across environments, and verified offline. Idempotency is enforced by re-hydrating envelopes before executing a capsule; if the payload matches, Demon proves nothing new happened and skips the duplicate. This lets teams retry failed automation, recover from transient outages, and build “approve once, run many” rituals without fear of double execution. The contract registry keeps schema changes honest, while replies and approval gates ensure humans stay in the loop exactly where their judgment matters.

## Agent-first Automation

- **Contracts stay ahead of code**: Capsule workflows negotiate via versioned JSON Schemas and WIT definitions stored in `contracts/`, keeping runtime compatibility across environments.
- **Envelopes make state portable**: Every capsule call, policy decision, and emitted event produces an envelope stored on JetStream, ready for replay, audit, or downstream automation.
- **Idempotent replays by default**: Rituals hydrate prior envelopes, compare their fingerprints, and only re-execute when inputs change—perfect for “retry without side effects” approvals.
- **Guardrails are built in**: Policy wards enforce quotas, time windows, and escalation paths, while approval gates provide human checkpoints with first-writer-wins semantics.

## Governance Guardrails

- **Required checks**: `Bootstrapper bundles — verify (offline, signature ok)`, `Bootstrapper bundles — negative verify (tamper ⇒ failed)`, `contracts-validate`, `review-lock-guard`, and `review-threads-guard (PR) / guard` must all stay green on `main`.
- **Review-lock discipline**: Every pull request body includes `Review-lock: <sha>` with the latest head SHA, updated on each push so provenance can be audited.
- **Replies policy**: All review comments receive explicit author responses before merge; no open threads remain when the branch lands.
- **Provenance verification**: Run `scripts/contracts-validate.sh` locally, then use `cargo run -p demonctl -- bootstrap --verify-only` (or the Alpha Preview Kit) to confirm signature integrity before promoting bundles.

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
  -d '{"approver": "ops@example.com", "note": "approved for production"}'
```

First-writer-wins semantics and TTL auto-deny keep approvals deterministic. See [docs/operate-ui/README.md](docs/operate-ui/README.md) for UI workflows and REST responses.

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
