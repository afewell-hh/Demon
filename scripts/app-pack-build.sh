#!/usr/bin/env bash

set -euo pipefail

if ! command -v cosign >/dev/null 2>&1; then
  echo "error: cosign CLI not found in PATH" >&2
  exit 1
fi

PACK_DIR=${1:-examples/app-packs/hoss}
COSIGN_KEY=${COSIGN_KEY:-$PACK_DIR/signing/cosign.key}
MANIFEST="$PACK_DIR/app-pack.yaml"
PUBLIC_KEY="$PACK_DIR/signing/cosign.pub"
SIGNATURE_BUNDLE="$PACK_DIR/signing/cosign.sig"

if [[ ! -f "$MANIFEST" ]]; then
  echo "error: manifest not found at $MANIFEST" >&2
  exit 1
fi

if [[ ! -f "$PUBLIC_KEY" ]]; then
  echo "error: public key not found at $PUBLIC_KEY" >&2
  exit 1
fi

if [[ ! -f "$COSIGN_KEY" ]]; then
  echo "error: private key not found. set COSIGN_KEY or place it under signing/" >&2
  exit 1
fi

echo "Signing bundle via cosign…"
cosign sign-blob \
  --key "$COSIGN_KEY" \
  --bundle "$SIGNATURE_BUNDLE" \
  "$MANIFEST"

PUBLIC_HASH=$(sha256sum "$PUBLIC_KEY" | awk '{print $1}')

python3 - "$MANIFEST" "$PUBLIC_HASH" <<'PY'
import re
import sys
from pathlib import Path

manifest_path = Path(sys.argv[1])
hash_value = sys.argv[2]

content = manifest_path.read_text()
pattern = r"(publicKeyHash:\s*\n\s+algorithm:\s*sha256\s*\n\s+value:\s*)([0-9a-fA-F]+)"
updated, count = re.subn(pattern, rf"\g<1>{hash_value}", content)

if count == 0:
    sys.exit("failed to rewrite publicKeyHash in manifest")

manifest_path.write_text(updated)
PY

echo "Verifying updated bundle with demonctl…"
DEMON_APP_HOME="$(mktemp -d)"
export DEMON_APP_HOME
trap 'rm -rf "$DEMON_APP_HOME"' EXIT
if ! demonctl app install "$PACK_DIR" >/tmp/app-pack-install.log 2>&1; then
  cat /tmp/app-pack-install.log >&2
  exit 1
fi
rm -rf "$DEMON_APP_HOME"
trap - EXIT

echo "Bundle signing complete. Updated manifest hash:${PUBLIC_HASH}"
