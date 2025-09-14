# ADR-0005: Approvals TTL via Timer Wheel

- Status: Accepted
- Date: 2025-09-09

## Context
Approvals gates should auto-deny when no terminal decision arrives before a TTL. We already emit `approval.requested:v1` and schedule an expiry using the in-process `TimerWheel`.

## Decision
- Use the existing TimerWheel to schedule per-gate expiry keys: `{runId}:approval:{gateId}:expiry`.
- On terminal (`approval.granted:v1` or `approval.denied:v1`) before TTL, cancel the expiry by key.
- Expose a lightweight counter to prove `cancel_by_key(..)` is invoked; tests assert this deterministically (no sleeps) using an injected clock.

## Consequences
- Deterministic unit tests verify auto-deny on TTL and preemption by terminal decisions.
- A future background worker is out-of-scope for M2C-2; if needed, a timer-wheel backed worker can be added later to survive process restarts.

