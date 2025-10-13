# Container Runtime Parity — Rootless Docker and containerd

Purpose
- Provide a quick validation guide to ensure Demon’s container-exec runtime behavior is consistent across rootless Docker and containerd.

Scope
- Flag semantics: `--cpus`, `--memory`, `--pids-limit` (from `DEMON_CONTAINER_*` envs).
- Network, read-only FS, `no-new-privileges`, tmpfs, user mapping.
- Envelope path writability under `/workspace/.artifacts`.

Prerequisites
- Rootless Docker installed and active for the current user.
- containerd with `nerdctl` (rootless if desired) or `ctr` and compatible image store.
- App Pack available locally (see `README.md` Quickstart) and NATS not required for this check.

Quick Validation (10–15 min)
- Set envs: `DEMON_CONTAINER_CPUS=0.5`, `DEMON_CONTAINER_MEMORY=256m`, `DEMON_CONTAINER_PIDS_LIMIT=128`.
- Run: `cargo test -p capsules_container_exec -- --nocapture` and confirm the ordering test passes.
- Observe `DEMON_DEBUG=1` with a simple run: `DEMON_DEBUG=1 cargo run -p demonctl -- run examples/rituals/echo.yaml`.
  - Confirm debug “runtime command:” shows flags before the image and includes `--network none`, `--read-only`, `--security-opt no-new-privileges`, `--user <uid:gid>`, and a `--tmpfs /tmp` entry.

Rootless Docker Notes
- CPU/memory limits require a recent kernel/cgroup v2; if unavailable, Docker may warn and ignore limits. This is expected; ensure flags are still placed pre-image.
- UID/GID mapping comes from the invoking user; verify with `id -u`/`id -g` in debug pre-run block.

containerd / nerdctl Notes
- Use `nerdctl run` parity: `--cpus`, `--memory`, and `--pids-limit` mirrors Docker CLI semantics when applied before the image.
- When testing outside Demon, replicate mounts and `--security-opt no-new-privileges` if supported in your environment.

Envelope Writability Check
- With `DEMON_DEBUG=1`, confirm `ENVELOPE_PATH` is writable and that diagnostics show host `ls -l` and `stat` for the mapped file.

Troubleshooting
- Limits ignored: check cgroup version and rootless support. Retry on a host with cgroup v2.
- Permission denied writing envelope: ensure App Pack `.artifacts/` path exists and host-side placeholder is present (Demon creates it; see code comments).
- Options after image: if you observe flags after the image, file an issue (regression violating test `resource_limits_flags_are_included_when_envs_set`).

Acceptance
- Both rootless Docker and containerd runs demonstrate: flags precede image; network/readonly/no-new-privileges/user/tmpfs present; envelope writable under `/workspace/.artifacts`.

