# App Pack Schema Reference (v1)

The App Pack manifest is declared in YAML (or JSON) and validated against `contracts/schemas/app-pack.v1.schema.json`. This document summarizes each field and the guarantees the platform enforces during installation.

## Top-level Fields

- `apiVersion` — Must be `demon.io/v1`. Aligns the manifest with a specific schema evolution track.
- `kind` — Always `AppPack`. Used to gate future manifest types.
- `metadata` — Identifiers for the bundle:
  - `name` — DNS-safe slug used as the registration namespace.
  - `version` — Semantic version of the pack (`MAJOR.MINOR.PATCH[-PRERELEASE]`).
  - `displayName` / `description` / `homepage` — Optional operator-facing metadata.
- `signing` — Optional signature verification. `signing.cosign` supports:
  - `keyRef` — Reference to a public key (file path, KMS, etc.).
  - `certificateIdentity` / `certificateIssuer` — Keyless verification matchers.
  - `rekorUrl` — Transparency log endpoint.
- `requires` — Declares compatible version ranges:
  - `appPackSchema` — Range string (e.g., `>=1.0.0 <2.0.0`).
  - `platformApis.engine` / `platformApis.runtime` / `platformApis.operateUi` — Semver range strings describing required platform API versions.
- `contracts` — Array of bundled contracts:
  - Each entry defines `id`, `version`, and `path` (relative within the bundle under `contracts/`).
- `capsules` — Array of capsule declarations the runtime can execute.
- `rituals` — Array of rituals exposed by the pack.
- `ui` — Optional manifest-driven Operate UI cards.

## Capsules

Each capsule entry describes a reusable execution primitive. For v1 the platform supports `type: container-exec` with the following fields:

- `name` — Stable identifier referenced by rituals.
- `imageDigest` — Digest-pinned container reference (`<registry>/<repo>@sha256:<digest>`).
- `command` — Array representing the container entrypoint.
- `env` — Optional map of string→string environment variables.
- `workingDir` — Optional working directory inside the container.
- `outputs.envelopePath` — Absolute path where the capsule writes the Explainable Result Envelope consumed by the runtime.

Capsules are sandboxed by the platform: non-root user, network disabled, read-only filesystem with a writable `tmpfs` at `/tmp`, and `no-new-privileges` enforced. Future schema revisions may add more capsule types.

## Rituals

A ritual defines an ordered list of steps:

- `name` — Unique identifier within the pack.
- `displayName` / `description` — Optional operator-facing metadata.
- `steps` — Non-empty array where each item includes:
  - `capsule` — Name of the capsule to invoke.
  - `with` — Arbitrary JSON object merged into the capsule input payload.

The runtime will ensure referenced capsules exist and will register the resulting rituals under the pack namespace.

## Operate UI Cards (`ui.cards`)

Cards enable the Operate UI to render manifest-driven views without shipping app-specific code:

- `id` — Card identifier.
- `kind` — Renderer key understood by Operate UI (`result-envelope` for the initial release).
- `title` — Display label (optional, but recommended).
- `match.rituals` — One or more ritual names the card applies to.
- `match.tags` — Optional run tags used for additional filtering.
- `fields.show` — List of envelope fields (JSON Pointer or dotted paths) to display in order.
- `fields.map` — Optional map of custom display labels to envelope field paths.

Operate UI will ingest these manifests at install time and render cards dynamically.

## Contracts

Contract entries reference JSON schema or fixture assets bundled with the pack. Paths must live under `contracts/` inside the bundle. Contracts are registered under the pack namespace during installation so they can be queried via the Contracts API.

## Versioning & Compatibility

- Schema versioning follows semver. Breaking changes increment the `MAJOR` component and require a new `apiVersion`.
- Additive fields (new optional properties) may be introduced within the same `MAJOR` stream.
- Apps must declare compatibility ranges via `requires`. The installer will refuse packs whose ranges conflict with the running platform.

Refer to `docs/app-packs/upgrade-policy.md` (to be added) for detailed compatibility guarantees.
