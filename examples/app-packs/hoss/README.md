# HOSS App Pack

This directory contains the signed App Pack bundle consumed by the HOSS
environment. The bundle mirrors the `contracts/schemas/app-pack.v1.schema.json`
contract and exercises the same Cosign verification flow enforced by
`demonctl`.

```
examples/app-packs/hoss/
├── app-pack.yaml              # Manifest (demon.io/v1)
├── contracts/hoss/contract.json
├── signing/
│   ├── cosign.pub             # PEM encoded public key (committed)
│   └── cosign.sig             # Cosign bundle (commit hash pinned)
└── README.md
```

## Re-signing the bundle

The committed `cosign.pub`/`cosign.sig` assets were generated with Cosign
against the exact contents of `app-pack.yaml`. Whenever the manifest changes,
regenerate the signature and hash metadata before publishing a new release:

```bash
# 1. Generate or rotate the signing key pair (writes cosign.key + cosign.pub)
cosign generate-key-pair \
  --key signing/cosign.key \
  --output signing/cosign

# 2. Sign the manifest (or the release tarball) and emit the bundle JSON
cosign sign-blob \
  --bundle signing/cosign.sig \
  --key signing/cosign.key \
  app-pack.yaml

# 3. Update the manifest hash metadata with the PEM digest
PUBLIC_HASH=$(sha256sum signing/cosign.pub | cut -d' ' -f1)
sed -i "s/^      value:.*/      value: ${PUBLIC_HASH}/" app-pack.yaml

# 4. Drop the private key before committing
rm signing/cosign.key
```

The `publicKeyHash.value` must be updated every time the public key rotates. CI
refuses to install the pack when the hash, signature, or bundle contents are out
of sync.

## Promotion checklist

- [ ] Validate schema compliance via `demonctl app install ./`.
- [ ] Run `cosign verify-blob` against the bundle to confirm the signature
      before promotion.
- [ ] Capture the git hash used for the release and update the promotion
      runbook.
- [ ] Publish the refreshed `app-pack.yaml`, `signing/cosign.pub`, and
      `signing/cosign.sig` artifacts together.
