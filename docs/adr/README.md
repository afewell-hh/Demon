# Architecture Decision Records (ADRs)

![Status: Current](https://img.shields.io/badge/Status-Current-green)

This directory contains Architecture Decision Records documenting important design decisions made during Demon's development.

## Overview

ADRs capture the rationale behind architectural choices, providing context for future maintainers and helping teams understand why decisions were made.

## ADR Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| *No ADRs yet* | *Initial ADRs will be documented here* | *Draft* | *TBD* |

**Status Legend:**
- **Proposed** - Under consideration
- **Accepted** - Approved and implemented
- **Deprecated** - No longer recommended
- **Superseded** - Replaced by newer ADR

ADRs capture the context, decision, and consequences of significant architectural choices. Each ADR follows a structured format to ensure decisions are well-documented and rationale is preserved for future reference.

## Current ADRs

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-0001](ADR-0001-meta-paas-scope.md) | Meta-PaaS Scope | Accepted | 2025-09 |
| [ADR-0002](ADR-0002-toolchain-policy.md) | Toolchain Policy | Accepted | 2025-09 |
| [ADR-0003](ADR-0003-wards-policy-and-approvals.md) | Wards Policy and Approvals | Accepted | 2025-09 |
| [ADR-0004](ADR-0004-wards-per-cap-quotas.md) | Wards Per-Cap Quotas | Accepted | 2025-09 |
| [ADR-0005](ADR-0005-approvals-ttl.md) | Approvals TTL | Accepted | 2025-09 |
| [ADR-0006](ADR-0006-ttl-worker.md) | TTL Worker | Accepted | 2025-09 |
| [ADR-0007](ADR-0007-bundle-library-and-provenance.md) | Bundle Library and Provenance | Accepted | 2025-09 |

## Key Architectural Decisions

### Core Platform
- **Meta-PaaS Scope** - Position as automation platform, not full PaaS
- **Rust Toolchain** - Modern systems language for performance and safety
- **Event Sourcing** - NATS JetStream for durable event persistence

### Security & Governance
- **Wards Policy System** - Automated policy enforcement and quota management
- **Approval Gates** - Human-in-the-loop workflows with TTL auto-deny
- **Bundle Provenance** - Cryptographic signatures for package security

### Operational Model
- **Per-Capability Quotas** - Resource limits and rate limiting
- **TTL Workers** - Automatic timeout handling for approvals
- **Bundle Library** - Secure package distribution system

## ADR Format

Each ADR follows this structure:

```markdown
# ADR-XXXX: Title

## Status
[Proposed | Accepted | Deprecated | Superseded]

## Context
[What situation led to this decision?]

## Decision
[What did we decide to do?]

## Consequences
[What are the positive and negative outcomes?]

## Alternatives Considered
[What other options were evaluated?]
```

## For Architects

When making significant architectural decisions:

1. **Research** existing ADRs for precedent
2. **Draft** new ADR using the standard format
3. **Review** with the team for feedback
4. **Accept** and implement the decision
5. **Update** status if later superseded

## For Developers

ADRs help you understand:
- **Why** certain patterns exist in the codebase
- **What** trade-offs were considered
- **How** to make consistent decisions
- **When** to revisit architectural choices

## For Evaluators

ADRs provide insight into:
- **Technical Maturity** - Thoughtful decision-making process
- **Architectural Vision** - Long-term thinking and planning
- **Risk Management** - Consideration of trade-offs and alternatives
- **Team Process** - Collaborative decision-making approach

---

**🔗 Related**: [MVP Contract](../mvp/01-mvp-contract.md) | [Evaluators Guide](../personas/evaluators.md) | [Governance](../governance/)