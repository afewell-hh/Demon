# Demon

[![CI](https://github.com/afewell-hh/demon/actions/workflows/ci.yml/badge.svg)](https://github.com/afewell-hh/demon/actions/workflows/ci.yml)
[![Preview: Alpha](https://img.shields.io/badge/Preview-Alpha-6f42c1.svg)](https://github.com/afewell-hh/demon/releases/tag/preview-alpha-1)

**Secure, auditable workflow automation with human approval gates and policy enforcement.**

Demon is a meta-platform that bridges the gap between rigid CI/CD pipelines and ungoverned automation.
Platform teams get programmable workflow control with built-in approval gates, policy enforcement, and
complete audit trails—without sacrificing developer velocity.

## What is Demon?

Demon provides three core pillars for controlled automation:

- **Ritual Engine**: Define workflows as declarative YAML "rituals" that execute with full event
  traceability
- **Policy Wards**: Enforce quotas, time windows, and approval requirements before any action executes
- **Approval Gates**: Human-in-the-loop controls with configurable TTLs and escalation paths

Unlike general workflow orchestrators, Demon is purpose-built for scenarios where governance, security,
and audit requirements are paramount—think production deployments, infrastructure changes, and
security-sensitive operations.

## Track C: Agent-First Automation

Modern platform teams face a new challenge: **AI agents are writing and executing automation at scale**.
Traditional CI/CD assumes humans write workflows once and run them many times. But when agents generate
workflows dynamically, you need:

- **Contract enforcement** — Agents must speak a versioned API (schemas, WIT definitions) so workflows
  remain compatible across updates
- **Event envelopes** — Every action produces a timestamped, schema-validated event for audit and
  replay; agents can't bypass the paper trail
- **Replay guarantees** — Deterministic event streams let you reconstruct what an agent did, why it
  decided to act, and roll back if needed

Demon treats **every workflow as data**: the ritual definition, the policy decisions, the approval
interactions—all captured in immutable event streams. This makes agent-driven automation safe to audit,
easy to debug, and simple to govern.

### Why This Matters

- **Agents as first-class actors**: When an agent generates a deployment ritual, Demon validates it
  against the contract registry, enforces policy wards, and logs every decision.
- **Replay for trust**: If an agent-triggered workflow fails, replay the event stream to see the exact
  sequence of actions. No guessing, no lost context.
- **Evolution without breakage**: Update your capsule logic or policy rules—contract versioning ensures
  old and new workflows coexist gracefully.

## Overview & Use Cases

### For Evaluators
- **Controlled Deployments**: Gate production releases behind approvals with automatic rollback on
  timeout
- **Infrastructure Automation**: Provision cloud resources with policy-enforced quotas and multi-level
  approvals
- **Security Workflows**: Automate incident response with mandatory security team sign-off
- **Agent-Driven Ops**: Let AI agents propose changes; Demon enforces human approval and policy checks
  before execution

### For Builders
- **WASM-Powered Capsules**: Write workflow logic in any language that compiles to WebAssembly
- **Event-Driven Architecture**: Every action generates structured events for monitoring and debugging
- **Contract Registry**: Versioned schemas ensure API compatibility across teams, environments, and
  agent-generated workflows
- **Envelope Protocol**: Standardized event payloads with metadata (timestamp, actor, signature) for
  provenance and replay

### For Operators
- **Real-Time Monitoring**: Web UI shows live workflow execution with complete event histories
- **Policy Management**: Configure quotas, approval chains, and timeout behaviors without code changes
- **Audit Compliance**: Immutable event streams provide complete audit trails for regulatory
  requirements
- **Replay & Debug**: Reconstruct any workflow run from its event stream—essential for agent-driven
  automation where you need to understand "what did it do?"

## Try Demon in 5 Minutes

See Demon in action with a simple workflow that showcases the ritual engine, event streams, and
approval flow:

```bash
# Start the development environment
make dev

# Run a sample ritual
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

**What you just saw:**
- The **ritual engine** interpreted the YAML workflow and executed the echo capsule
- **Events** were published to NATS JetStream with full traceability (`ritual.started`,
  `ritual.completed`)
- **Policy decisions** were evaluated (quotas, approval gates) before execution
- A **JSON event stream** captured the entire workflow lifecycle for audit and replay
- Each event follows the **envelope protocol**: schema version, timestamp, actor metadata

The echo ritual is intentionally simple—real workflows can orchestrate complex approval chains, enforce
time-based policies, and integrate with existing CI/CD systems or agent-driven automation platforms.

### Bootstrap for Self-Hosting

Deploy Demon in your environment with zero-config setup:

```bash
# Complete bootstrap (stream + events + UI verification)
cargo run -p demonctl -- bootstrap --ensure-stream --seed --verify

# Individual steps available
cargo run -p demonctl -- bootstrap --ensure-stream    # Create NATS stream
cargo run -p demonctl -- bootstrap --seed            # Seed sample events
cargo run -p demonctl -- bootstrap --verify          # Verify Operate UI

# Environment overrides supported
RITUAL_STREAM_NAME=PROD_STREAM cargo run -p demonctl -- bootstrap --ensure-stream
```

See [docs/bootstrapper/README.md](docs/bootstrapper/README.md) for production deployment guides.

## Core Capabilities

- **🔒 Approval Gates**: REST API for granting/denying approvals with first-writer-wins semantics and
  TTL auto-deny
- **📊 Policy Engine**: Configurable quotas, time windows, and approval chains with real-time policy
  decisions
- **🎯 Event Persistence**: All workflow actions stored in NATS JetStream with deterministic replay
  capability
- **🖥️ Operate UI**: Real-time dashboard showing runs, events, and approval status with filtering and
  search
- **📦 Contract Registry**: Versioned schemas and WIT definitions for API compatibility and integration
- **🚀 Self-Hosting**: Zero-config bootstrap for NATS streams, seed data, and UI verification
- **🔁 Replay Protocol**: Reconstruct any workflow run from its immutable event stream for debugging
  and compliance

## Current Release Status

**Alpha Preview**: All M0 capabilities complete and battle-tested. The platform successfully handles
basic rituals, approval workflows, and policy enforcement in development environments.

**Coming in Beta**: Enhanced UI dashboard, multi-tenant support, and advanced policy engine with
escalation chains.

**Production Readiness**: Planned for M2 with hardened security, scale testing, and operational
runbooks.

→ [**Try the Alpha Preview Kit**](docs/preview/alpha/README.md)

## Architecture & Components

- **`engine/`** — Ritual interpreter that executes workflows and emits events
- **`runtime/`** — WASM capsule runtime with link-name routing for secure execution
- **`demonctl/`** — CLI for running rituals, managing contracts, and bootstrapping environments
- **`operate-ui/`** — Web dashboard for monitoring runs and managing approvals
- **`contracts/`** — JSON schemas and WIT definitions for API contracts
- **`capsules/echo/`** — Reference WASM capsule demonstrating the runtime interface

### Event Envelope Format

Every Demon event follows a standardized envelope:

```json
{
  "schema_version": "1.0.0",
  "event_type": "ritual.started",
  "timestamp": "2025-10-02T14:32:10Z",
  "actor": "agent-id-or-user-email",
  "payload": { /* event-specific data */ },
  "metadata": {
    "run_id": "uuid",
    "correlation_id": "uuid",
    "signature": "optional-cryptographic-proof"
  }
}
```

This structure enables:
- **Provenance**: Who triggered the action (human or agent)?
- **Replay**: Deterministic reconstruction of workflow history
- **Validation**: Schema-checked payloads prevent malformed events
- **Audit**: Immutable log of every decision and action

## Vision & Community

Demon aims to become the standard for governed automation in cloud-native environments. We're building
toward:

- **Universal Integration**: Native connectors for major CI/CD platforms, cloud providers, and security
  tools
- **Policy-as-Code**: Git-managed policy definitions with automated testing and deployment
- **Enterprise Features**: Advanced audit reporting, compliance frameworks, and organizational controls
- **Agent Marketplace**: Curated library of AI agents that safely compose Demon rituals with built-in
  policy enforcement

**Get Involved**:
- 📋 [View our roadmap and current milestones](https://github.com/users/afewell-hh/projects/1)
- 🐛 [Report issues or request features](https://github.com/afewell-hh/demon/issues)
- 📖 [Read our documentation](docs/)
- 💬 [Join discussions](https://github.com/afewell-hh/demon/discussions)

## Development

```bash
# Build and test the workspace
make build && make test

# Format and lint (required for CI)
make fmt && make lint

# Quick smoke test
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

**API Examples**:

```bash
# Grant approval (first-writer-wins)
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/grant \
  -H "Content-Type: application/json" \
  -d '{"approver": "ops@example.com", "note": "approved for production"}'

# Fetch contract bundles
GH_TOKEN=your_token cargo run -p demonctl -- contracts fetch-bundle

# Export all contracts
cargo run -p demonctl -- contracts bundle --format json --include-wit
```

## Project Process

- **MVP Contract**: [docs/mvp/01-mvp-contract.md](docs/mvp/01-mvp-contract.md) — Problem definition and
  M0 must-haves
- **Branch Protection**: [docs/process/branch_protection_mvp.md](docs/process/branch_protection_mvp.md)
  — Required CI checks and review policies
- **Project Board**: [GitHub Project](https://github.com/users/afewell-hh/projects/1) — Stories, epics,
  and milestone tracking

---

## Preview Kit & Resources

- 🔧 [Alpha Preview Kit](docs/preview/alpha/README.md) — Hands-on evaluation guide
- 📦 [Bundle Library & Signatures](docs/bootstrapper/bundles.md) — Offline verification and CI
  enforcement
- 📋 [Contract Bundle Releases](docs/contracts/releases.md) — Automated schema distribution

<sub>**Local verification**:</sub>
```bash
target/debug/demonctl bootstrap --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
| jq -e 'select(.phase=="verify" and .signature=="ok")' >/dev/null && echo "signature ok"
```
