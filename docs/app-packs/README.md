# Demon App Packs

App Packs are portable, signed bundles that register capsules, rituals, contracts, and Operate UI manifest cards onto the Demon platform. They keep the platform app-agnostic by describing all integration points declaratively.

## Bundle Layout

An App Pack is a tarball or directory with the following structure:

```
/app-pack.yaml         # Manifest matching contracts/schemas/app-pack.v1.schema.json
/contracts/            # JSON contracts bundled with the pack
/capsules/             # Optional helper assets referenced by capsules
/ui/                   # Optional static assets referenced by documentation (never shipped to Demon)
```

Only the manifest and contract assets are required by the platform. Additional artifacts (scripts, READMEs) may be included for operator convenience but are ignored during installation.

## Lifecycle Overview

1. **Authoring** — Build a manifest that conforms to the App Pack schema, pinning all container images by digest and declaring rituals, capsules, and UI cards.
2. **Signing** — Optionally sign the manifest with Cosign and include verification settings (key reference or certificate constraints) in `signing.cosign`.
3. **Distribution** — Publish the bundle to an OCI registry or make it available over HTTPS.
4. **Installation** — Operators run `demonctl app install <uri>`; the CLI downloads, validates, and registers the pack.
5. **Operation** — Rituals become available through the Demon runtime and Operate UI renders cards via manifest-driven configuration.
6. **Uninstallation** — Operators run `demonctl app uninstall <name>` to revoke registrations and clean up resources.

## Next Steps

- Read `docs/app-packs/schema.md` for a field-by-field description of the manifest.
- Use the upcoming `demonctl app` commands to install, list, and uninstall packs.
- Coordinate with the App Pack consumer (e.g., HOSS) to align on schema and API version ranges before publishing.
