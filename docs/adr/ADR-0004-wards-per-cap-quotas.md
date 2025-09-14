# ADR-0004 — Wards Per-Capability Quotas

## Context

Tenant/global quotas are insufficient when different capabilities under the same tenant need independent limits.

## Decision

- Introduce cap-level quotas via `WARDS_CAP_QUOTAS` with precedence over `WARDS_QUOTAS` and `WARDS_GLOBAL_QUOTA`.
- Counters are tracked per `(tenant, capability)` with a fixed window.
- Emit `policy.decision:v1` including camelCase quota block `{ limit, windowSeconds, remaining }`.
- Omit `decision.reason` when allowed; set to `"limit_exceeded"` when denied.

## Known Limitations (M2C-1)

- Counters are process-local only. Multi-instance deployments may drift until a distributed counter is implemented (targeted for M2D).
- Kernel is a singleton (`OnceLock<Mutex<_>>`) using a monotonic clock for windows; critical section is small (increment + occasional window reset).

## Consequences

- Per-capability independence: two capabilities for the same tenant do not share remaining quota.
- Engine/state replay remains stable; policy decisions do not alter ritual state.

## Status

Accepted.

