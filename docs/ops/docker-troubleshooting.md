# Docker Troubleshooting & Performance

**📍 [Home](../README.md) › [Operations](README.md) › Docker Troubleshooting & Performance**

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Rapid fixes for Docker build failures, GHCR publishing issues, and slow pipeline runs.

## Quick Links

- [Docker Build & Publish Guide](../how-to-guides/docker-pipeline.md)
- [CI Docker Workflow](../../.github/workflows/docker-build.yml)
- [Command Cheat Sheet — Docker Section](../quick-reference/command-cheat-sheet.md#docker--containers)
- Logs directory: `logs/docker-*.log`

## Common Failure Modes

### 1. Cargo Build Failures Inside Docker

**Symptoms:** `error: could not compile` during the `cargo build --release` stage.

**Fix:**
- Run `make build` locally to surface errors before invoking Docker.
- Re-run the workflow with a clean cache: `gh workflow run docker-build.yml -f component=runtime --ref <branch>` then retrigger after fixing compile errors.
- If the failure mentions missing `crate-type`, ensure the crate has a corresponding binary target.

### 2. GHCR Authentication or 403 Errors

**Symptoms:** `failed to fetch anonymous token: unexpected status: 403 Forbidden`.

**Fix:**
- **CI:** Authentication uses `GITHUB_TOKEN` automatically. Verify Packages: write permission in workflow logs.
- **Local push:**
  ```bash
  docker login ghcr.io -u <github-username>
  docker tag demon-local/runtime:dev ghcr.io/afewell-hh/demon-runtime:dev
  docker push ghcr.io/afewell-hh/demon-runtime:dev
  ```
- Check GHCR status page when failures cluster across jobs.
- See `GHCR_FIX_SUMMARY.md` for historical context and mitigations.

### 3. Cache Misses & Slow Rebuilds

**Symptoms:** Build job exceeds 60 minutes, repeated dependency compilation.

**Fix:**
- Confirm cache hits in logs: search for `Using cache` sections in the Buildx step.
- Warm caches by running the workflow on a feature branch before merging to `main`.
- Avoid force-pushing large dependency changes; incremental merges preserve cache layers.
- For local builds, mount cargo cache:
  ```bash
  docker build \
    --build-arg CARGO_HOME=/workspace/.cargo \
    -f runtime/Dockerfile .
  ```

### 4. Smoke Test Pull Failures

**Symptoms:** `ImagePullBackOff` in `scripts/tests/smoke-k8s-bootstrap.sh`.

**Fix:**
- Ensure GHCR images exist: `docker manifest inspect ghcr.io/afewell-hh/demon-runtime:main`.
- Provide pre-built locals when iterating quickly:
  ```bash
  export LOCAL_IMAGE_IMPORT="demon-local/runtime:dev demon-local/engine:dev demon-local/operate-ui:dev"
  scripts/tests/smoke-k8s-bootstrap.sh --verbose --cleanup
  ```
- For private registries, extend bootstrap config with `registries[]` as documented in `GHCR_FIX_SUMMARY.md`.

### 5. Buildx Builder Drift

**Symptoms:** `failed to find builder` or mismatched BuildKit version.

**Fix:**
- Remove stale builders locally: `docker buildx rm $(docker buildx ls | awk '/demon/{print $1}')`.
- Reset builder inside CI rerun (workflow step already removes builder on cleanup). If drift persists, re-run with `workflow_dispatch`.

## Performance Benchmarks (Alpha Preview)

| Run ID | Date (UTC) | Context | Runtime | Engine | Operate UI |
|--------|------------|---------|---------|--------|------------|
| 17964002949 | 2025-09-24 | First GHCR publish (cold cache) | 64.3 min | 64.9 min | 78.5 min |
| 18020300302 | 2025-09-25 | PR validation (warm cache) | 6.9 min | 6.8 min | 10.8 min |

**Takeaways:**
- Expect the first push after massive dependency updates to take over an hour.
- Warmed caches bring total matrix wall-clock under 15 minutes.
- Cache scope is per component (`scope=${{ matrix.component }}`), so touching only one crate keeps other images fast.

## Image Size Snapshot (linux/amd64)

| Component | Digest | Compressed Size |
|-----------|--------|----------------|
| Runtime | `sha256:d106f63b…eca53` | ≈1.9 MiB |
| Engine | `sha256:c47cee22…a5c6` | ≈1.9 MiB |
| Operate UI | `sha256:7ddea572…316f` | ≈10.1 MiB |

**How to verify:**
```bash
docker manifest inspect ghcr.io/afewell-hh/demon-runtime:main | jq '.manifests[0].digest'
docker manifest inspect ghcr.io/afewell-hh/demon-runtime@sha256:<digest> | jq '.layers[].size'
```

## Diagnostic Commands

- Tail latest workflow logs: `rg --no-heading -n "error:" logs/docker-*.log`
- Inspect GHCR tags: `gh api /orgs/afewell-hh/packages/container/demon-runtime/versions`
- Verify Buildx builder: `docker buildx inspect --bootstrap`
- Confirm cache key usage: `rg "cache-from" -n .github/workflows/docker-build.yml`

## Escalation Checklist

1. Capture failing workflow URL and attach relevant `logs/docker-*.log` snippet.
2. Note whether `docker-build.yml` ran from PR or `main` (push enables pushes).
3. Include digests from the last successful run (`logs/docker-publish-*.log`).
4. Update `docs/releases/README-HANDOFF.md` if production manifests are blocked.

---

**Next steps when blockers persist:** open Issue #186 (or successor) with logs attached, ping platform lead on Slack, and rerun `docker-build.yml` once root cause is resolved.
