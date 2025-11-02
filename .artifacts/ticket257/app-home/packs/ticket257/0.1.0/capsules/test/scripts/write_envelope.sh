#!/usr/bin/env bash
set -euo pipefail
umask 077
echo "[capsule] writing envelope to $ENVELOPE_PATH" >&2
printf '{"result":{"success":true,"data":{"note":"entrypoint-ok"}},"diagnostics":[]}' > "$ENVELOPE_PATH"
