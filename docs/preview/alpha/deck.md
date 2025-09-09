# Client Deck — Preview Alpha (5 Slides)

## Slide 1 — Title
- Demon Meta‑PaaS (Alpha Preview)
- Platform to build platforms.
- Preview tag: `preview-alpha-1` (27e36b21136e)

## Slide 2 — Why Demon
- Compose domain platforms fast (vPaaS) with capsules (WASM) and rituals (workflows).
- Deterministic by design: durable timers, idempotent events, replay.
- Policy everywhere: quotas, approvals, TTL; human‑in‑the‑loop.
- Cloud‑agnostic: wasmCloud/NATS first. K8s is an adapter later.

## Slide 3 — What’s in Alpha
- Operate UI (read): runs list + detail; incidents ready later.
- Event Log: JetStream persistence + deterministic replay.
- Wards (Policy): deny‑by‑default, per‑cap quotas, approvals, TTL auto‑deny.
- Preview Kit: seeder, runbook, CI smoke for reproducibility.

## Slide 4 — Show Me (Demo Map)
- Seed 3 runs with one script.
- Open `/runs` → pick A/B/C.
- A: `policy.decision:v1` allow → deny (quota).
- B: `approval.requested` → `approval.granted` (REST).
- C: `approval.requested` → TTL auto‑denied (worker).
- All events camelCase; idempotent keys; refresh is consistent.

## Slide 5 — Next Up
- M3+: Capsule registry, provenance/signatures, Operate UI actions.
- K8s bootstrapper (optional).
- Multi‑node quotas ADR & rollout.
- Call to action: pick a domain spike (e.g., data ingest vPaaS) for the Beta track.

