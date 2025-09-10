# Preview Alpha — Presenter Script (60‑sec)

- Context (5s)
  - “This is Demon: a Meta‑PaaS to build platform‑as‑a‑service for your domain. Today is a deterministic, 10‑minute alpha preview.”
- Setup (5s)
  - “We’re on tag `preview-alpha-1` (27e36b21136e). NATS + Operate UI + TTL worker are running. I’ll seed three demo runs.”
- Runs list (10s)
  - Navigate to `/runs`: “Here are recent runs persisted in JetStream. Everything you see is replayable.”
- Run A: Policy (15s)
  - Open run A: “Policy decisions (Wards) apply quotas per capability. You’ll see `policy.decision:v1` allow → deny with a camelCase quota block.”
- Run B: Approvals (15s)
  - Open run B: “Approvals are first‑writer‑wins and idempotent. The seeder requested, then we granted via REST — note `approval.granted:v1`.”
- Run C: TTL (10s)
  - Open run C: “Pending approvals auto‑deny on TTL. The worker emits `approval.denied:v1` with `reason:"expired"`.”
- Close (5s)
  - “These flows are deterministic: idempotent message IDs, durable timers, persistent event log. Next step is your domain spike for Beta.”

## URLs to click
- Runs list: http://localhost:3000/runs
- Run B detail: http://localhost:3000/runs/run-preview-b
- Run C detail: http://localhost:3000/runs/run-preview-c
