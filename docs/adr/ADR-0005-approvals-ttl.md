# ADR-0005: Approvals TTL via Timer Wheel

Status: Accepted (2025-09-08)

## Context

We need an automatic deny when an approval gate remains pending beyond a configured TTL. We already have an event log and a lightweight timer wheel abstraction. There is no long‑running background worker in M2C‑2.

## Decision

- Extend the engine approval hook to schedule an expiry timer when emitting `approval.requested:v1`.
- Use idempotent keys based on `(runId, gateId)`.
- On expiry, emit `approval.denied:v1` with `{ "reason": "expired", "approver": "system" }` using the existing denied contract.
- Terminal guards: if a grant/deny already exists, the expiry is a no‑op.
- UI surfaces the state as “Denied — expired”.

## Implementation Notes

- Env: `APPROVAL_TTL_SECONDS` (default `0` = disabled).
- Timer ID: `"{runId}:approval:{gateId}:expiry"`.
- Idempotency keys:
  - request: `"{runId}:approval:{gateId}"`
  - expiry scheduled: `"{runId}:approval:{gateId}:expiry:scheduled"`
  - auto‑deny: `"{runId}:approval:{gateId}:denied"`
- M2C‑2 does not include a background scheduler. Integration tests simulate the wheel firing by calling `process_expiry_if_pending(..)` after TTL.
- Cancellation: providing `cancel_by_key(..)` marks timers delivered and logs a one‑liner; unit tests assert behavior deterministically without sleeps.

## Risks

- Time‑based flake in integration tests. Mitigated via short TTL and bounded waits; unit tests are deterministic.
- Double‑fire on replay: prevented by idempotency keys and terminal guard on readback.

## Future Work (M2C‑3)

- Add a background worker to drive `timer.scheduled` → `process_expiry_if_pending(..)` without tests calling it.
- Persist timer specs and cancellations durably.
- Observability: metrics for scheduled, canceled, auto‑denied counts.
