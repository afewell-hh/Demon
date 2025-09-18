# Bundle Library and Offline Verify

- URI formats:
  - `lib://local/{name}@{version}` resolves against `bootstrapper/library/index.json` (local provider)
  - `lib://https/{name}@{version}` resolves via HTTPS from remote registry (https provider)
- Index schema: `contracts/schemas/bootstrap.library.index.v0.json`.
- Provider types:
  - `local`: Bundles stored in filesystem relative to index
  - `https`: Bundles fetched from remote HTTPS server using `baseUrl` + `path`
- Env interpolation: `${VAR}` and `${VAR:-default}` before canonicalization.
- Canonicalization: YAML → JSON Value → recursively sort all maps (BTreeMap) → `serde_json::to_vec` bytes.
- Digest: SHA-256 over canonical bytes; lower-case hex.
- Signature: Ed25519 over canonical bytes; public key base64 at `contracts/keys/{pubKeyId}.ed25519.pub`.

Verify-only examples

Local bundle:
```
target/debug/demonctl bootstrap --bundle lib://local/preview-local-dev@0.0.1 --verify-only
```

Remote bundle (HTTPS):
```
target/debug/demonctl bootstrap --bundle lib://https/preview-local-dev@0.0.1 --verify-only
```

Expected JSON lines

```
{"phase":"resolve", ...}
{"phase":"verify","bundle":{"name":"preview-local-dev","version":"0.0.1"},"digest":"<sha256>","signature":"ok","pubKeyId":"preview"}
```

File map

- `bootstrapper/library/index.json` — local provider index.
- `contracts/schemas/bootstrap.library.index.v0.json` — schema (supports both local and https providers).
- `contracts/keys/preview.ed25519.pub` — public key (base64). No private keys committed.
- `contracts/provenance/*.sha256` and `*.sig` — fixtures.

## Remote Bundle Registry (HTTPS Provider)

When using the HTTPS provider:
- The index specifies `"provider": "https"` and a `"baseUrl"` field
- Bundles are fetched from `<baseUrl>/<path>` where path is from the bundle entry
- Downloads are cached in a temporary directory for the session (not persisted)
- The canonical digest is computed and verified against the index entry
- Signature verification uses the same flow as local bundles
- HTTP errors, connection failures, or digest mismatches fail immediately

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

#### Key rotation & multiple keys

- Multiple public keys: keep older keys alongside new ones in `contracts/keys/`. Each library index entry must specify `pubKeyId` indicating which key was used to sign that bundle.
- Rotation flow: add the new key → re‑sign bundles with the new key → update `bootstrapper/library/index.json` entries with the new `pubKeyId` → keep the previous key checked in for a deprecation window so `--verify-only` still passes for older tags.
- CI is offline and verifies against whichever `pubKeyId` the index declares — no network trust is involved.

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

Determinism checklist

- Quote ambiguous YAML scalars (on, off, yes, no, timestamps) to avoid unintended type coercion.
- Numbers: prefer integers or quoted strings; avoid leading zeros.
- Whitespace: remove trailing spaces; ensure LF line endings.
- Interpolation order: `${VAR}` is resolved before canonicalization — set envs explicitly when verifying locally.
- Arrays/maps: authoring order of maps doesn’t matter; canonicalization sorts map keys lexicographically.

Local one‑liner verify

```bash
cargo build --locked -q && \
target/debug/demonctl bootstrap --verify-only --bundle lib://local/preview-local-dev@0.0.1 | \
jq -r 'select(.phase=="verify")'
```

Required job name reminder: do not rename

- The required check is pinned as: `Bootstrapper bundles — negative verify (tamper ⇒ failed)`.
