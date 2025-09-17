#!/usr/bin/env bash
set -euo pipefail

# Config
# Honor NATS_PORT if provided; default 4222
if [[ -z "${NATS_URL:-}" && -n "${NATS_PORT:-}" ]]; then
  NATS_URL="nats://127.0.0.1:${NATS_PORT}"
fi
NATS_URL=${NATS_URL:-nats://127.0.0.1:4222}
RITUAL_STREAM_NAME=${RITUAL_STREAM_NAME:-RITUAL_EVENTS}
UI_URL=${UI_URL:-http://127.0.0.1:3000}

export NATS_URL RITUAL_STREAM_NAME

ritual=preview
tenant=default
now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

RUN_A=${RUN_A:-run-preview-a}
RUN_B=${RUN_B:-run-preview-b}
RUN_C=${RUN_C:-run-preview-c}
GATE_B=${GATE_B:-gate-b}
GATE_C=${GATE_C:-gate-c}

subject() { echo "demon.ritual.v1.$1.$2.events"; }
expiry_key() { echo "$1:approval:$2:expiry"; }

# Ensure UI is reachable (retry for ~20s)
for i in {1..40}; do
  if curl -sf "$UI_URL/healthz" >/dev/null 2>&1 || curl -sf "$UI_URL/api/runs" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo "Seeding Run A (policy allow -> deny)"
pol_allow=$(jq -n --arg ts "$(now)" --arg t "$tenant" --arg run "$RUN_A" --arg rit "$ritual" '{event:"policy.decision:v1",ts:$ts,tenantId:$t,runId:$run,ritualId:$rit,decision:{effect:"allow",reason:"quota ok"}}')
pol_deny=$(jq -n --arg ts "$(now)" --arg t "$tenant" --arg run "$RUN_A" --arg rit "$ritual" '{event:"policy.decision:v1",ts:$ts,tenantId:$t,runId:$run,ritualId:$rit,decision:{effect:"deny",reason:"quota exceeded"}}')
cargo run -q -p engine --bin demon-seed -- "$(subject $ritual $RUN_A)" "$pol_allow"  "$RUN_A:policy:1"
cargo run -q -p engine --bin demon-seed -- "$(subject $ritual $RUN_A)" "$pol_deny"   "$RUN_A:policy:2"

echo "Seeding Run B (approval requested -> grant via REST)"
req_b=$(jq -n --arg ts "$(now)" --arg t "$tenant" --arg run "$RUN_B" --arg rit "$ritual" --arg gate "$GATE_B" '{event:"approval.requested:v1",ts:$ts,tenantId:$t,runId:$run,ritualId:$rit,gateId:$gate,requester:"dev@example.com",reason:"promote"}')
cargo run -q -p engine --bin demon-seed -- "$(subject $ritual $RUN_B)" "$req_b" "$RUN_B:approval:$GATE_B"
curl -sS -X POST "$UI_URL/api/approvals/$RUN_B/$GATE_B/grant" \
  -H 'content-type: application/json' \
  -H 'X-Requested-With: XMLHttpRequest' \
  -d '{"approver":"ops@example.com","note":"ok"}' >/dev/null

echo "Seeding Run C (approval requested -> TTL scheduled; worker will auto-deny)"
req_c=$(jq -n --arg ts "$(now)" --arg t "$tenant" --arg run "$RUN_C" --arg rit "$ritual" --arg gate "$GATE_C" '{event:"approval.requested:v1",ts:$ts,tenantId:$t,runId:$run,ritualId:$rit,gateId:$gate,requester:"dev@example.com",reason:"promote"}')
cargo run -q -p engine --bin demon-seed -- "$(subject $ritual $RUN_C)" "$req_c" "$RUN_C:approval:$GATE_C"
timer_id=$(expiry_key "$RUN_C" "$GATE_C")
scheduled_for=$(date -u -d "+5 seconds" +%Y-%m-%dT%H:%M:%SZ)
timer=$(jq -n --arg ts "$(now)" --arg run "$RUN_C" --arg tid "$timer_id" --arg due "$scheduled_for" '{event:"timer.scheduled:v1",ts:$ts,runId:$run,timerId:$tid,scheduledFor:$due}')
cargo run -q -p engine --bin demon-seed -- "$(subject $ritual $RUN_C)" "$timer" "$RUN_C:approval:$GATE_C:expiry:scheduled"

echo "Preview seed complete. Summary:"
echo "- Subject: $(subject $ritual $RUN_A) | Run A: $RUN_A (policy allow -> deny)"
echo "- Subject: $(subject $ritual $RUN_B) | Run B: $RUN_B (approvals grant)"
echo "- Subject: $(subject $ritual $RUN_C) | Run C: $RUN_C (approvals TTL auto-deny)"
