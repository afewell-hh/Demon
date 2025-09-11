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

