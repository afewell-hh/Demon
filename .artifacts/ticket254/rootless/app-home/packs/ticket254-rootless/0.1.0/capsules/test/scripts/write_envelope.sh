#!/usr/bin/env bash
set -euo pipefail
umask 077
printf '{"result":{"success":true,"data":{"note":"rootless"}},"diagnostics":[]}' > "$ENVELOPE_PATH"
