# App Pack Installer Guarantees

The Demon installer enforces several invariants to keep App Pack deployments
safe. The most important additions in this iteration are the Cosign signature
checks that execute before any bundle contents are registered.

## Cosign Verification Workflow

When `signing.cosign.enabled` is `true` (the default), the installer expect the
following assets inside the bundle:

- `signing/cosign.sig` (or whichever path you provide via
  `signing.cosign.signaturePath`) — a Cosign signature or bundle produced with
  `cosign sign --bundle …`.
- `signing/cosign.pub` (`signing.cosign.publicKeyPath`) — the PEM encoded public
  key corresponding to the private key used at signing time.
- `signing.cosign.publicKeyHash` — an object declaring the hashing algorithm
  (`sha256`) and the hex-encoded digest of the PEM contents.

During installation Demon will:

1. Load the PEM key, compute the SHA-256 digest, and compare it to
   `publicKeyHash.value`. A mismatch aborts the install.
2. Verify the signature/bundle against the source manifest using
   `sigstore-verification`. Any tampering (altered manifest, signature, or key)
   aborts the install before contracts, rituals, or UI metadata are registered.

If `enabled` is set to `false`, the installer skips signature validation. The
boolean shorthand (`signing.cosign: true`) is now rejected to avoid ambiguous
configurations.

## Additional Guarantees

- Manifests are validated against `contracts/schemas/app-pack.v1.schema.json`.
- Capsules must declare digest-pinned images and output destinations.
- Ritual and UI references are cross-checked to prevent missing dependencies.

Future releases will extend the signature workflow to cover keyless Fulcio
certificates and Rekor transparency log enforcement. The schema already
reserves fields (`certificateIdentity`, `certificateIssuer`, `rekorUrl`) for
that evolution.
