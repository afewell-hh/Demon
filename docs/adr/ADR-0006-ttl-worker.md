# ADR-0006: TTL Worker (Durable Consumer)

- Status: Accepted
- Date: 2025-09-09

## Context
Approvals with TTL should auto-deny at due time without manual intervention. JetStream carries timer.scheduled:v1 events on the ritual events subject.

## Decision
- Add a minimal single-threaded worker that consumes `timer.scheduled:v1` from a durable pull consumer (`TTL_CONSUMER_NAME`, default `ttl-worker`).
- Filter to `demon.ritual.v1.*.*.events` and parse only timer ids of the form `{runId}:approval:{gateId}:expiry`.
- Call `process_expiry_if_pending(..)` and ack on success or terminal-noop; leave unacked on error for retry.

## Consequences
- At-least-once processing with idempotent effects per run/gate.
- Restart-safe via durable consumer.

## Config
- `TTL_WORKER_ENABLED=0|1` (default off)
- `RITUAL_STREAM_NAME` (optional; falls back to `RITUAL_EVENTS` then `DEMON_RITUAL_EVENTS`)
- `TTL_CONSUMER_NAME` (default `ttl-worker`)
- `TTL_BATCH` (default 100), `TTL_PULL_TIMEOUT_MS` (default 1500)

## Out of Scope
- Horizontal scaling and per-tenant sharding (future slice).

