# Demon App Packs

App Packs are portable, signed bundles that register capsules, rituals, contracts, and Operate UI manifest cards onto the Demon platform. They keep the platform app-agnostic by describing all integration points declaratively.

## Bundle Layout

An App Pack is a tarball or directory with the following structure:

```
/app-pack.yaml         # Manifest matching contracts/schemas/app-pack.v1.schema.json
/contracts/            # JSON contracts bundled with the pack
/capsules/             # Optional helper assets referenced by capsules
/signing/             # Cosign signature bundle (cosign.sig, cosign.pub)
/ui/                   # Optional static assets referenced by documentation (never shipped to Demon)
```

Only the manifest and contract assets are required by the platform. Additional artifacts (scripts, READMEs) may be included for operator convenience but are ignored during installation.

## Lifecycle Overview

1. **Authoring** — Build a manifest that conforms to the App Pack schema, pinning all container images by digest and declaring rituals, capsules, and UI cards.
2. **Signing** — Optionally sign the bundle with Cosign, storing the signature under `signing/` alongside the PEM public key and a `publicKeyHash` entry in `signing.cosign`.
3. **Distribution** — Publish the bundle to an OCI registry or make it available over HTTPS.
4. **Installation** — Operators run `demonctl app install <uri>`; the CLI downloads, validates, and registers the pack.
5. **Operation** — Rituals become available through the Demon runtime and Operate UI renders cards via manifest-driven configuration.
6. **Uninstallation** — Operators run `demonctl app uninstall <name>` to revoke registrations and clean up resources.

## Next Steps

- Read `docs/app-packs/schema.md` for a field-by-field description of the manifest.
- Review `docs/app-packs/installer-guarantees.md` to understand the signature
  validation guarantees enforced by the installer.
- Use the `demonctl app` commands to install, list, and uninstall packs.
- Use the `demonctl app` commands to install, list, and uninstall packs, then run
  rituals via `demonctl run <app>:<ritual>` (optionally `app@version:ritual`).
- Coordinate with the App Pack consumer (e.g., HOSS) to align on schema and API
  version ranges before publishing.
