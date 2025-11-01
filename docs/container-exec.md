# Container-Exec Capsule Runtime

The `container-exec` capsule provides a hardened bridge between Demon rituals and
container images shipped in App Packs. It is responsible for wiring sandbox
flags, preparing writable artifact mounts, and converting capsule output into
Explainable Result Envelopes that the platform can validate and replay.

## Sandbox Defaults

Every container launched by `container-exec` is constrained with the following
flags:

- `--network none`
- `--read-only`
- `--tmpfs /tmp:rw,noexec,nosuid,nodev,size=64m`
- `--security-opt no-new-privileges`
- `--user <uid:gid>` (defaults to the invoking user or `65534:65534`)
- Digest-pinned images (`imageDigest` must include `@sha256:<digest>`)

The runtime prepares a writable `.artifacts/` mount and ensures the target
envelope file is created with permissive permissions before execution so
non-root containers can write results even when the workspace is read-only.

## Runtime Selection

The container runtime binary is chosen via `DEMON_CONTAINER_RUNTIME`:

- `stub` – loads an envelope from `DEMON_CONTAINER_EXEC_STUB_ENVELOPE` and skips
  container execution. Useful for offline tests.
- Any other value – treated as the runtime binary to invoke (default: `docker`).

Failures to spawn the runtime yield an error envelope with code
`CONTAINER_EXEC_RUNTIME_ERROR` and include captured stdout/stderr diagnostics.

## Execution Timeout

Capsules can declare a timeout in the App Pack manifest using
`timeoutSeconds`. When set, the runtime terminates the container if it exceeds
the allotted time and returns an error envelope with code
`CONTAINER_EXEC_TIMEOUT`. Operators can override the timeout globally using the
environment variable `DEMON_CONTAINER_EXEC_TIMEOUT_SECONDS`.

## Diagnostics and Observability

`container-exec` annotates emitted envelopes with:

- Exit status of the runtime command (warning when non-zero)
- Captured stdout/stderr (trimmed to 2 KiB) from the runtime
- Optional debug diagnostics when `DEMON_DEBUG` is non-empty, including the
  resolved command line and host-side `ls`/`stat` of the envelope path

All envelopes are validated before being returned to the runtime. Missing or
invalid envelopes produce canonical error envelopes (`CONTAINER_EXEC_ENVELOPE_*`
codes) with the relevant logs attached for troubleshooting.

## Testing With the Stub Runtime

For deterministic tests, set:

```bash
export DEMON_CONTAINER_RUNTIME=stub
export DEMON_CONTAINER_EXEC_STUB_ENVELOPE=/path/to/result-envelope.json
```

The runtime will bypass container execution and return the provided envelope
after validation. This is the preferred mode for unit tests and CI environments
without Docker/Podman.
