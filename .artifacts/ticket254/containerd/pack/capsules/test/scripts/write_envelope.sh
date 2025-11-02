#!/usr/bin/env bash
set -euo pipefail
umask 077
printf '{"result":{"success":true,"data":{"note":"containerd"}},"diagnostics":[]}' > "$ENVELOPE_PATH"
