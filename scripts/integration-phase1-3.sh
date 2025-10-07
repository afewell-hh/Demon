#!/usr/bin/env bash

set -euo pipefail

PACK_DIR=${1:-examples/app-packs/hoss}
OUTPUT_FILE=${2:-integration-phase1-3.json}

if [[ ! -f "$PACK_DIR/app-pack.yaml" ]]; then
  echo "error: no app-pack.yaml found under $PACK_DIR" >&2
  exit 1
fi

TMP_HOME=$(mktemp -d)
trap 'rm -rf "$TMP_HOME"' EXIT

echo "[phase-1] demonctl app install"
if DEMON_APP_HOME="$TMP_HOME" cargo run -q -p demonctl -- app install "$PACK_DIR" >"$TMP_HOME/install.log" 2>&1; then
  phase1_status=success
  phase1_error=""
else
  phase1_status=failure
  phase1_error="$(tail -n 20 "$TMP_HOME/install.log" | tr '\n' ' ')"
fi

APP_NAME=$(awk '/^metadata:/ {flag=1; next} flag && /^[^ ]/ {exit} flag && /name:/ {print $2; exit}' "$PACK_DIR/app-pack.yaml")
APP_NAME=${APP_NAME:-hoss}
RITUAL="${APP_NAME}:noop"

echo "[phase-2] demonctl run $RITUAL"
if DEMON_APP_HOME="$TMP_HOME" cargo run -q -p demonctl -- run "$RITUAL" >"$TMP_HOME/run.log" 2>&1; then
  phase2_status=success
  phase2_error=""
else
  phase2_status=failure
  phase2_error="$(tail -n 20 "$TMP_HOME/run.log" | tr '\n' ' ')"
fi

echo "[phase-3] cosign verify-blob"
if cosign verify-blob \
    --key "$PACK_DIR/signing/cosign.pub" \
    --bundle "$PACK_DIR/signing/cosign.sig" \
    "$PACK_DIR/app-pack.yaml" >"$TMP_HOME/cosign.log" 2>&1; then
  phase3_status=success
  phase3_error=""
else
  phase3_status=failure
  phase3_error="$(tail -n 20 "$TMP_HOME/cosign.log" | tr '\n' ' ')"
fi

cat >"$OUTPUT_FILE" <<EOF
{
  "phase1": {"status": "${phase1_status}", "error": "${phase1_error}"},
  "phase2": {"status": "${phase2_status}", "error": "${phase2_error}"},
  "phase3": {"status": "${phase3_status}", "error": "${phase3_error}"}
}
EOF

echo "dashboard payload -> $OUTPUT_FILE"
