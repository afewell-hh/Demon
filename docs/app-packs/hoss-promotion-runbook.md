# HOSS App Pack Promotion Runbook

Status: Draft (MVP alpha)

This runbook captures the checklist for preparing and publishing a new HOSS App
Pack bundle. Follow these steps whenever the manifest, contracts, or signing
materials change.

## 1. Prepare the bundle

- [ ] Update `examples/app-packs/hoss/app-pack.yaml` with the desired metadata
      and ritual definitions.
- [ ] Refresh any contract fixtures under `examples/app-packs/hoss/contracts/`.
- [ ] Run `demonctl app install examples/app-packs/hoss` to validate schema
      compliance and signature verification locally.
- [ ] Exercise the alias path via `demonctl run hoss:noop` to ensure rituals
      load correctly.

## 2. Regenerate signing artifacts

Regenerate the Cosign bundle after every manifest change or key rotation:

```bash
pushd examples/app-packs/hoss

# Generate/rotate key pair (writes signing/cosign.key + cosign.pub)
cosign generate-key-pair --key signing/cosign.key --output signing/cosign

# Sign the manifest (or release tarball) and emit a detached bundle
cosign sign-blob \
  --key signing/cosign.key \
  --bundle signing/cosign.sig \
  app-pack.yaml

# Update the manifest hash metadata
PUBLIC_HASH=$(sha256sum signing/cosign.pub | cut -d' ' -f1)
sed -i "s/^      value:.*/      value: ${PUBLIC_HASH}/" app-pack.yaml

# Sanity check
cosign verify-blob \
  --key signing/cosign.pub \
  --bundle signing/cosign.sig \
  app-pack.yaml

# Drop the private key before committing
rm signing/cosign.key

popd
```

## 3. Publish

- [ ] Commit `app-pack.yaml`, `signing/cosign.pub`, and `signing/cosign.sig`
      together.
- [ ] Capture the git SHA associated with the release in the deployment notes.
- [ ] Attach the updated App Pack archive to the HOSS promotion request.
- [ ] Notify the Demon operators and attach verification logs (output from
      `demonctl app install` and `cosign verify-blob`).

## 4. Post-promotion

- [ ] Update the MVP progress tracker with the release link.
- [ ] Audit the Cosign public key expiration and schedule rotation at least
      14 days before expiry.
- [ ] Capture the first green nightly review-kit execution after the
      artifact rename (Issue #28) and attach logs to the promotion record.
