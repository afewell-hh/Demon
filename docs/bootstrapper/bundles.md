Bundle Library and Offline Verify

- URI format: `lib://local/{name}@{version}` resolves against `bootstrapper/library/index.json`.
- Index schema: `contracts/schemas/bootstrap.library.index.v0.json`.
- Env interpolation: `${VAR}` and `${VAR:-default}` before canonicalization.
- Canonicalization: YAML → JSON Value → recursively sort all maps (BTreeMap) → `serde_json::to_vec` bytes.
- Digest: SHA-256 over canonical bytes; lower-case hex.
- Signature: Ed25519 over canonical bytes; public key base64 at `contracts/keys/{pubKeyId}.ed25519.pub`.

Verify-only example

```
target/debug/demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only
```

Expected JSON lines

```
{"phase":"resolve", ...}
{"phase":"verify","bundle":{"name":"preview-local-dev","version":"0.0.1"},"digest":"<sha256>","signature":"ok","pubKeyId":"preview"}
```

File map

- `bootstrapper/library/index.json` — local provider index.
- `contracts/schemas/bootstrap.library.index.v0.json` — schema.
- `contracts/keys/preview.ed25519.pub` — public key (base64). No private keys committed.
- `contracts/provenance/*.sha256` and `*.sig` — fixtures.

## CI verification: positive & negative

We run two **offline** verification jobs for bundle provenance:

- **Bootstrapper bundles — verify (offline)**  
  Builds `demonctl` and runs:  
  `demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only`  
  Asserts a JSON line with `{"phase":"verify","signature":"ok"}`.

- **Bootstrapper bundles — negative verify (tamper ⇒ failed)** (required)  
  Deterministically tampers `examples/bundles/local-dev.yaml` (e.g., bumps `duplicateWindowSeconds`), then runs the same `--verify-only`.  
  Expects `{"phase":"verify","signature":"failed"}` and a **non-zero** exit.

Why both?  
The positive job proves the committed digest+signature match; the negative job proves verification actually fails on content drift (prevents “always green” false positives).

Notes
- Both jobs are **offline** (no NATS/UI). They won’t flake on infra.
- The negative job name is **pinned**. **Do not rename**:  
  `Bootstrapper bundles — negative verify (tamper ⇒ failed)`
- Local quick checks:
  ```bash
  # Positive (ok)
  cargo build --locked --workspace
  target/debug/demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only \
    | jq -e 'select(.phase=="verify" and .signature=="ok")' >/dev/null

  # Negative (expected fail)
  cp examples/bundles/local-dev.yaml{,.bak}
  awk '/duplicateWindowSeconds/{sub(/[0-9]+/,"121")}1' examples/bundles/local-dev.yaml > /tmp/b.yaml && mv /tmp/b.yaml examples/bundles/local-dev.yaml
  if target/debug/demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only; then
    echo "ERROR: verify unexpectedly succeeded" && exit 1
  fi
  mv examples/bundles/local-dev.yaml{.bak,}
  ```

### Troubleshooting

**“signature”: “failed” but no local changes**
- Cause: line endings (CRLF vs LF) changed the canonical bytes.
- Check:
  ```bash
  git config --get core.autocrlf
  file -b examples/bundles/local-dev.yaml
  ```

Fix:

```bash
# one-time normalize the repo to LF
printf "* text=auto\n" >> .gitattributes
git add --renormalize .
git commit -m "chore: normalize line endings to LF"
# or convert just the bundle file
dos2unix examples/bundles/local-dev.yaml  # on macOS: brew install dos2unix
```

Env interpolation surprises (${VAR} / ${VAR:-default})

Symptom: verify passes locally but fails in CI.

Check the phase=config line for effective + provenance to confirm the final values used.

Fix: set the variables explicitly during --verify-only, or ensure defaults in the bundle:

```bash
FOO=bar target/debug/demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only
```

Index/schema errors

Symptom: index schema invalid, unknown bundle, or No such file.

Check: bootstrapper/library/index.json is valid against contracts/schemas/bootstrap.library.index.v0.json and paths are correct.

Quick local guard:

```bash
cargo test -p bootstrapper-demonctl -- libindex_spec -- --nocapture
```

CI “negative verify” fails unexpectedly

Ensure the job name is unchanged (required check):
Bootstrapper bundles — negative verify (tamper ⇒ failed)

The job deterministically bumps duplicateWindowSeconds; verify your bundle still contains that field.

If you tested locally, make sure you reverted any tamper:

```bash
git checkout -- examples/bundles/local-dev.yaml
```

jq not found (local runs)

Install: brew install jq (macOS) or sudo apt-get install -y jq (Debian/Ubuntu).

Windows shells

Prefer Git Bash or WSL for the documented commands; PowerShell may need adjusted quoting.
