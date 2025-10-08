# Container Exec Capsule

The container-exec capsule provides a sandboxed execution primitive for Demon
App Packs. It runs digest-pinned container images with hardened runtime flags,
collects stdout/stderr for diagnostics, and materializes Explainable Result
Envelopes from a shared bind mount.

## Features

- Enforces digest-pinned images (`@sha256:<digest>`)
- Locks down the container (`--network=none`, `--read-only`, `--tmpfs /tmp`,
  `--security-opt=no-new-privileges`, non-root user)
- Captures stdout/stderr and exit code as diagnostics
- Reads the result envelope from the declared `outputs.envelopePath`
- Validates the envelope against the platform schema
- Emits canonical error envelopes when the runtime, envelope, or configuration
  fail
- Supports a `stub` runtime for local testing (set `DEMON_CONTAINER_RUNTIME=stub`
  and point `DEMON_CONTAINER_EXEC_STUB_ENVELOPE` to an envelope JSON file)

## Envelope Write Semantics (non-root containers)

When running capsules as a real user (e.g., `--user 1000:1000`) with a hardened
container profile (read-only root, `--network=none`, `--tmpfs /tmp`,
`--security-opt=no-new-privileges`), the capsule must still be able to write the
result envelope.

The runtime guarantees this by:

- Pre-creating `/workspace/.artifacts` in the App Pack and creating the target
  envelope file path there with permissive modes (dirs `0777`, file `0666`).
- Mounting the App Pack at `/workspace` as read-only and binding the artifacts
  directory as read-write at `/workspace/.artifacts`.
- Adding a direct file-level bind from the host envelope placeholder to the
  container target path (rw). This ensures the envelope is written even when the
  parent mount is read-only or owned by another UID.

Notes:

- The only writable locations are `/workspace/.artifacts/<file>` (the bound
  file) and `/tmp` (tmpfs). No other paths are relaxed.
- You can override the container user via `DEMON_CONTAINER_USER` (default
  `65534:65534`). The runtime’s writability guarantees hold for non-root UIDs.

Troubleshooting:

- If Docker (or your runtime) requires the target path to exist for file binds,
  ensure the App Pack was prepared via `demonctl app install` (the runtime will
  pre-create the container-side target automatically during execution).
  Inspect the host-side artifacts directory for the bound file if debugging.

## Usage

```rust
use capsules_container_exec::{execute, ContainerExecConfig};
use std::collections::BTreeMap;

let config = ContainerExecConfig {
    image_digest: "ghcr.io/example/app@sha256:...".into(),
    command: vec!["/bin/run".into()],
    env: BTreeMap::new(),
    working_dir: None,
    envelope_path: "/workspace/.artifacts/result.json".into(),
    capsule_name: Some("sample".into()),
    app_pack_dir: Some(std::path::PathBuf::from("/path/to/app-pack")),
    artifacts_dir: Some(std::path::PathBuf::from("/tmp/demon-run/artifacts")),
};

let envelope = execute(&config);
// Serialize envelope back to the runtime caller
```

## Environment Overrides

- `DEMON_CONTAINER_RUNTIME` — container binary to execute (default: `docker`).
  Set to `stub` when running without a container runtime.
- `DEMON_CONTAINER_EXEC_STUB_ENVELOPE` — path to an envelope JSON file that is
  returned when `DEMON_CONTAINER_RUNTIME=stub`.
- `DEMON_CONTAINER_USER` — user (`uid:gid`) to run containers as (default:
  `65534:65534`).
- Provide `workspaceDir` / `artifactsDir` in the request (or via
  `ContainerExecConfig`) so the App Pack is mounted read-only at `/workspace`
  and result artifacts are written to `/workspace/.artifacts`.

## Future Work

- Optional support for additional capsule outputs (artifacts, logs)
- Toggleable rootless runtimes (e.g., `nerdctl`, `podman`)
- Streaming log diagnostics back to callers
