# Preview Checklist (Alpha)

Entry
- NATS JetStream reachable (4222/8222)
- operate-ui running at http://127.0.0.1:3000
- TTL worker enabled (env TTL_WORKER_ENABLED=1)

Exit (acceptance)
- Seed script completes with run IDs and subjects
- /api/runs returns 200 and lists runs (array; `jq 'length >= 1'`)
- Granted flow present (approval.requested -> approval.granted)
- TTL auto-deny present (exactly one approval.denied with reason:"expired")
- /runs and /runs/<id> render without error banners
- /admin/templates/report shows template_ready=true and has_filter_tojson=true

Non-goals
- Scaling, multi-tenant shards, or long-lived scheduling

Troubleshooting
- Port clashes: set NATS_PORT and NATS_URL
- No data: re-run seed script; it’s idempotent via Nats-Msg-Id
