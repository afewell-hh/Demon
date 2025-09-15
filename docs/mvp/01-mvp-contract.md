# Demon MVP — Contract

## Problem & Personas
- Problem: Platform teams need secure, auditable workflow automation with human approval gates and policy enforcement
- Primary personas: Platform engineers, DevOps teams, security engineers requiring controlled automation

## Must-have Capabilities (M0)
- [ ] M0-1: Basic Ritual Execution — Acceptance: Can run `cargo run -p demonctl -- run examples/rituals/echo.yaml`, echo capsule prints "Hello from Demon!", JSON event for `ritual.completed:v1` is emitted
- [ ] M0-2: Event Persistence to JetStream — Acceptance: Events published to `demon.ritual.v1.<ritualId>.<runId>.events` subject, includes `ritual.started:v1`/`ritual.state.transitioned:v1`/`ritual.completed:v1`, `nats stream info RITUAL_EVENTS` shows non-zero message count, deterministic replay capability
- [ ] M0-3: Policy Decisions (Wards) — Acceptance: Support `WARDS_CAP_QUOTAS` configuration, emit `policy.decision:v1` events with camelCase quota block `{limit, windowSeconds, remaining}`, allow → deny transitions when quotas exceeded, reason field shows `null` for allow, `"limit_exceeded"` for deny
- [ ] M0-4: Approval Gates — Acceptance: Emit `approval.requested:v1` events, REST API for granting approvals, first-writer-wins semantics (200 OK vs 409 conflict), emit `approval.granted:v1` or `approval.denied:v1` events, idempotent approval resolution
- [ ] M0-5: TTL Auto-Deny — Acceptance: TTL worker consumes timer events from JetStream, automatic emission of `approval.denied:v1` with `reason:"expired"`, configurable `APPROVAL_TTL_SECONDS`, timer cancellation on terminal decisions
- [ ] M0-6: Operate UI - Runs List — Acceptance: `/runs` endpoint lists all runs, `/runs/<id>` shows detailed run timeline, pages render without template errors, UI displays events in chronological order
- [ ] M0-7: REST API for Runs — Acceptance: `/api/runs` returns array of runs (≥1), `/api/runs/<id>` returns detailed run with events, API supports filtering and querying events, JSON responses properly formatted
- [ ] M0-8: Development Environment — Acceptance: `make dev` command starts NATS and builds workspace, Docker Compose configuration for NATS JetStream, configurable ports (`NATS_PORT=4222`, `NATS_MON_PORT=8222`), seed script for demo data

## Should-have (M1)
- [ ] M1-1: Enhanced UI Dashboard — Acceptance: Real-time event streaming, filtering and search capabilities, approval action buttons in UI
- [ ] M1-2: Multi-tenant Support — Acceptance: Namespace isolation, per-tenant quotas and policies
- [ ] M1-3: Advanced Policy Engine — Acceptance: Time-based policies, complex approval workflows, escalation chains

## Non-goals
- Full workflow orchestration (use existing tools like GitHub Actions/GitLab CI)
- General-purpose messaging platform (NATS JetStream handles this)
- User management and authentication (integrate with existing systems)

## Release Criteria
- [ ] All M0 checked
- [ ] CI green on main (all required checks)
- [ ] "replies-guard" required + passing
- [ ] Docs: README quickstart + HOWTOs updated
- [ ] Smoke demo script passes (Playwright)

## Risks & Open Questions
- NATS JetStream operational complexity in production environments
- Approval UI UX for complex approval scenarios
- Policy engine extensibility vs simplicity trade-offs
