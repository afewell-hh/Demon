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
 - Clears image `ENTRYPOINT` (`--entrypoint ""`) so the capsule's declared
   command is executed exactly as provided. This avoids Docker's
   `ENTRYPOINT + CMD` concatenation which can break read-only filesystems or
   wrapper shells.

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
 - "script not found under /workspace/capsules": the container command in your
   capsule likely references a path like `/workspace/capsules/<name>/scripts/*.sh`.
   This path exists only when the App Pack tree is installed and mounted at
   `/workspace` (read-only). Fix by installing the pack with `demonctl app install`
   and invoking the ritual via `demonctl run <app>:<ritual>`. Do not reference the
   source tree paths directly — the runtime resolves capsule scripts relative to
   the installed App Pack mounted at `/workspace`.

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

### Entrypoint Behavior

Some images define an `ENTRYPOINT` like `/bin/bash -lc`. By default, Docker
concatenates `ENTRYPOINT` and the runtime `CMD`, altering how commands execute
and sometimes preventing writes on hardened, read-only filesystems. The
container-exec capsule explicitly passes `--entrypoint ""` before the image
reference so that only the capsule's `command` runs. Images without an
`ENTRYPOINT` are unaffected.

## Environment Overrides

- `DEMON_CONTAINER_RUNTIME` — container binary to execute (default: `docker`).
  Set to `stub` when running without a container runtime.
- `DEMON_CONTAINER_EXEC_STUB_ENVELOPE` — path to an envelope JSON file that is
  returned when `DEMON_CONTAINER_RUNTIME=stub`.
- `DEMON_CONTAINER_USER` — user (`uid:gid`) to run containers as (default:
  `65534:65534`).
- `DEMON_DEBUG` — when set to a non-empty value other than `0`, enables
  additional diagnostics and a lightweight debug wrapper around the declared
  command. The wrapper prints a pre-run banner with the effective UID/GID,
  the resolved `ENVELOPE_PATH`, a short `ls -l` of the envelope directory, and
  a snapshot of the container mount table. The runtime also includes the full
  container command line (showing `--entrypoint ""` and mounts), plus host-side
  `ls`/`stat` of the bound envelope file, in the emitted envelope diagnostics.
  - Provide `workspaceDir` / `artifactsDir` in the request (or via
  `ContainerExecConfig`) so the App Pack is mounted read-only at `/workspace`
  and result artifacts are written to `/workspace/.artifacts`.

Resource limits:

- `DEMON_CONTAINER_CPUS` — passed to `docker run --cpus` (e.g., `0.5`).
- `DEMON_CONTAINER_MEMORY` — passed to `docker run --memory` (e.g., `256m`).
- `DEMON_CONTAINER_PIDS_LIMIT` — passed to `docker run --pids-limit` (e.g., `128`).

All configured limits are reflected in the emitted DEMON_DEBUG runtime command line.

## Future Work

- Optional support for additional capsule outputs (artifacts, logs)
- Toggleable rootless runtimes (e.g., `nerdctl`, `podman`)
- Streaming log diagnostics back to callers
