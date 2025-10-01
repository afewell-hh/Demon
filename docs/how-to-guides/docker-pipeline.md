# Docker Build & Publish Guide

**📍 [Home](../README.md) › [How-to Guides](README.md) › Docker Build & Publish Guide**

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Build, tag, and publish Demon container images locally and through the GitHub Container Registry (GHCR) workflow.

## Overview

- **Audience**: Demon developers shipping runtime, engine, or operate-ui changes
- **Goal**: Produce local images for smoke testing and understand the automated GHCR pipeline
- **Scope**: Local Docker builds, GHCR publishing flow, manual workflow triggers, and smoke test imports

> 🔗 See also: [Docker Troubleshooting & Performance Notes](../ops/docker-troubleshooting.md)

## Prerequisites
- Docker Engine or Docker Desktop 24.x or newer with BuildKit enabled
- `docker buildx` plugin (installed automatically with recent Docker releases)
- Access to the Demon workspace with the Rust toolchain installed (`make build` succeeds)
- Optional: GitHub CLI (`gh`) for triggering workflows and viewing build logs

Verify your environment:

```bash
docker version | head -n 1
docker buildx version
make build        # ensures workspace compiles before baking images
```

## Local Image Builds

Each core component ships with a multi-stage Dockerfile in its crate directory.

```bash
# Build individual component images (linux/amd64)
docker build -f engine/Dockerfile -t demon-local/engine:dev .
docker build -f runtime/Dockerfile -t demon-local/runtime:dev .
docker build -f operate-ui/Dockerfile -t demon-local/operate-ui:dev .

# Optional: multi-platform build with buildx (requires QEMU)
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f runtime/Dockerfile \
  -t demon-local/runtime:multiarch \
  .
```

### Import Local Images into Smoke Tests

Set the `LOCAL_IMAGE_IMPORT` environment variable when running `scripts/tests/smoke-k8s-bootstrap.sh` to preload images into the ephemeral k3d cluster:

```bash
export LOCAL_IMAGE_IMPORT="demon-local/runtime:dev demon-local/engine:dev demon-local/operate-ui:dev"
scripts/tests/smoke-k8s-bootstrap.sh --verbose --cleanup
```

The script calls `k3d image import` internally. Provide space-separated image references that are already present in your local Docker cache.

## Automated GHCR Workflow

The GitHub Actions workflow [`docker-build.yml`](../../.github/workflows/docker-build.yml) builds all three images on every pull request and push to `main`.

### Trigger Matrix

| Trigger | Behavior |
|---------|----------|
| Pull request touching Dockerfiles/workspace | Builds images, **no push** |
| Push to `main` | Builds images, pushes tags to GHCR |
| `workflow_dispatch` | Manual run with the same behavior as the invoked event |

The job matrix builds the `engine`, `runtime`, and `operate-ui` images independently using Docker Buildx with GitHub Actions cache scopes per component.

### Tags and Naming

- Registry prefix: `ghcr.io/afewell-hh/demon-<component>`
- Tags published on `main` pushes:
  - `latest` (default branch only)
  - `sha-<git-sha>` (immutable digest reference)
  - Branch name aliases (e.g., `main`, `feature/my-fix`)
- Pull requests receive the same tag set but remain in the workflow cache and are **not** pushed to GHCR.

### Authentication Notes

- The workflow authenticates with `secrets.GITHUB_TOKEN`; no additional PAT is required.
- For local pushes to GHCR, log in using a personal access token with `write:packages`:

```bash
docker login ghcr.io -u <github-username> -p <ghcr_pat>
docker build -f runtime/Dockerfile -t ghcr.io/afewell-hh/demon-runtime:dev .
docker push ghcr.io/afewell-hh/demon-runtime:dev
```

### Manual Invocations

```bash
# Trigger the workflow manually (default inputs)
gh workflow run docker-build.yml

# Watch build progress
gh run watch --exit-status --job build --workflow docker-build.yml

# Inspect image metadata emitted by the workflow logs
gh run view --log $(gh run list --workflow docker-build.yml --limit 1 --json databaseId -q '.[0].databaseId')
```

## Verification Checklist

- [ ] `make build` completes before building containers
- [ ] Local `docker build` succeeds for the component you touched
- [ ] `LOCAL_IMAGE_IMPORT` includes all locally tagged images when running bootstrap smoke tests
- [ ] GitHub Actions workflow run is green (build job per component)
- [ ] Image tags are visible under GHCR packages for `afewell-hh`

## Related Resources

- [Docker Troubleshooting & Performance Notes](../ops/docker-troubleshooting.md)
- [Command Cheat Sheet](../quick-reference/command-cheat-sheet.md)
- [K8s Bootstrap Smoke Test Script](../../scripts/tests/smoke-k8s-bootstrap.sh)
- [CI Docker Build Workflow](../../.github/workflows/docker-build.yml)
