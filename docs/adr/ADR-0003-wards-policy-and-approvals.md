# ADR-0003 — Wards Policy and Approvals (M2A/M2B)

Status: Draft (M2A in review, M2B in progress)

- Fixed-window quotas with a single global windowSeconds (60s default for M2x; revisit per-tenant in later ADR).
- Deny-by-default: missing capability or misconfiguration results in denial and a clear reason.
- Decision event idempotency: Nats-Msg-Id derives from run scope (e.g., `{runId}:decision:{capability}`) to ensure exactly-once persistence within JetStream’s dedup window.
- Approvals single-resolution rule: first terminal resolution wins (grant or deny); duplicates are no-ops via idempotency + state check.
- Reuse ritual stream for auditability: all policy and approval events publish to `demon.ritual.v1.<ritualId>.<runId>.events` for a linear, queryable run history.

