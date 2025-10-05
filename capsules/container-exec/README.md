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

## Future Work

- Optional support for additional capsule outputs (artifacts, logs)
- Toggleable rootless runtimes (e.g., `nerdctl`, `podman`)
- Streaming log diagnostics back to callers
