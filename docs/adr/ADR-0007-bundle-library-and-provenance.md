ADR-0007: Bundle Library and Offline Provenance

Decision

- Introduce a local bundle library (`lib://local/{name}@{version}`) with index validation.
- Provide an offline `--verify-only` path that resolves, canonicalizes, digests, and verifies signatures without NATS or network.

Canonicalization Sequence

- Load YAML and interpolate `${VAR}` / `${VAR:-default}`.
- Convert to `serde_json::Value`.
- Recursively sort all maps (use `BTreeMap`), recursing into arrays.
- Serialize with `serde_json::to_vec` to produce canonical bytes.
- Compute SHA-256 over these bytes; hex lowercase.

Signature & Trust Model

- Sign canonical bytes with Ed25519. Verification uses committed public keys in `contracts/keys/*.ed25519.pub` (base64).
- Private keys are never committed; a local helper `sign_bundle` is gated behind `--features dev-tools` for developer testing only.
- CI enforces offline verification: build with `--locked`, run `--verify-only`, and require `signature: "ok"` in JSON output.

Alternatives Considered

- YAML canonicalization alone (rejected): ambiguity across parsers; JSON + map sort is deterministic.
- Embedding signatures in the YAML (rejected): mutates the signed content; we keep signatures out-of-band in the library index.

