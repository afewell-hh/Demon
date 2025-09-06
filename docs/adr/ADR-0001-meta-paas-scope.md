ADR-0001: Demon Meta-PaaS Scope & Principles

Date: YYYY-MM-DD
Status: Proposed
Context: Project ignition (Milestone 0)

Decision

We will implement Demon as a wasmCloud-native Meta-PaaS: a platform to build platforms (vPaaS). Demon will ship a runtime kernel, workflow engine, data plane, policy/tenancy layer, Operate UI, and a self-host bootstrapper — all cloud-agnostic, modular, and interface-first.

Demon will start as a monorepo, containing all core subsystems and contracts. This supports cohesive early development, single-pipeline CI, and contract-driven evolution. We reserve the option to split capsules/SDKs into separate repos later.

Scope

In-scope:

Capsule runtime and link routing.

Workflow interpreter for Serverless Workflow–style rituals.

Durable data plane (NATS + JetStream).

Policy/tenancy (“wards”).

Operate UI (observability of workflows).

Bootstrapper CLI (demonctl) with local/remote profiles.

Capsule SDK (Rust first).

Contracts (WIT + JSON Schema) and registry stubs.

Out-of-scope (initial):

Kubernetes bootstrapper (optional later).

AI/LLM dependencies (agent-ready, not agent-required).

Domain-specific integrations (keep APIs generic).

Monolithic services (favor capsule modularity).

Principles

Interface-First: every subsystem publishes contracts (WIT, JSON Schema, OpenAPI if HTTP).

Determinism & Idempotency: workflows and capsules must behave predictably; all side-effects go through capsules.

Durability by Default: workflow engine uses event-sourced state, sagas, and durable timers.

Policy Everywhere: wards enforce capability, tenancy, quotas, approvals.

Observability: structured logs, traces, replayable execution state, and first-class Operate UI.

Cloud-Agnostic: wasmCloud + NATS first; Kubernetes and other substrates as adapters.

Small Diffs, Small Modules: prefer capsule-sized units with feature flags and minimal commits.

Rationale

By declaring Demon as a Meta-PaaS rather than a domain PaaS, we establish a durable foundation: generic, composable, and usable by any team to build their own vertical platform. The monorepo simplifies early bootstrapping and quality gates. Strict adherence to contracts and small-diff philosophy ensures the system is agent-friendly and resilient.

Consequences

Demon can evolve to support multiple vPaaS use cases without re-architecture.

Early milestones are achievable in small, testable slices.

Optional Kubernetes adapter and AI agent add-ons remain possible without polluting the core.

Repo layout may need to be revisited once capsule registry and SDKs mature.