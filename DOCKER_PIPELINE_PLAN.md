# Docker Pipeline Implementation Plan

## Executive Summary

This document outlines the implementation plan for a complete Docker image build and publish pipeline to replace the temporary `nginx:alpine` placeholder images currently used in the K8s bootstrapper. The pipeline will build and publish Docker images for `operate-ui`, `runtime`, and `engine` components to GitHub Container Registry (GHCR).

## Current State Analysis

### Confirmed Issues
- ✅ Nightly validation (run 17959076204) confirmed placeholder images eliminate ImagePullBackOff errors
- ✅ All 6/6 pods reach Ready state with `nginx:alpine` placeholders
- ✅ Core K8s deployment infrastructure is working correctly
- ❌ Health checks fail due to placeholder containers not exposing expected endpoints

### Missing Infrastructure
- **No Dockerfiles** for any component (operate-ui, runtime, engine)
- **No CI workflow** to build and push Docker images
- **No GHCR authentication** configured for image publishing
- **No image versioning strategy** defined

## Implementation Roadmap

### Phase 1: Core Docker Infrastructure (Week 1)

#### 1.1 Create Dockerfiles
Create multi-stage Dockerfiles for each component following Rust best practices:

**Files to create:**
- `operate-ui/Dockerfile`
- `runtime/Dockerfile`
- `engine/Dockerfile`

**Dockerfile requirements:**
- Multi-stage build (builder + runtime)
- Rust nightly toolchain support
- Minimal runtime image (distroless or alpine)
- Proper layer caching for dependencies
- Security best practices (non-root user, minimal attack surface)

#### 1.2 Docker Build Workflow
Create `.github/workflows/docker-build.yml`:

**Trigger conditions:**
- Push to `main` branch
- Pull request changes affecting Docker-related files
- Manual dispatch for testing

**Build matrix:**
- Component: [operate-ui, runtime, engine]
- Platform: [linux/amd64, linux/arm64] (if multi-arch needed)

**Workflow steps:**
1. Checkout code
2. Set up Docker Buildx
3. Login to GHCR using GITHUB_TOKEN
4. Build and tag images
5. Push to GHCR on main branch
6. Output image metadata for consumption

### Phase 2: GHCR Integration (Week 1-2)

#### 2.1 Authentication Strategy
**Repository Settings:**
- Use built-in `GITHUB_TOKEN` with `packages: write` permission
- No additional secrets needed (leverages GitHub's native GHCR integration)

**Image visibility:**
- Public images (recommended for CI simplicity)
- Alternative: Private with proper pull secrets if security required

#### 2.2 Image Naming Convention
```
ghcr.io/afewell-hh/demon-operate-ui:latest
ghcr.io/afewell-hh/demon-operate-ui:v1.0.0
ghcr.io/afewell-hh/demon-operate-ui:sha-{git-sha}

ghcr.io/afewell-hh/demon-runtime:latest
ghcr.io/afewell-hh/demon-runtime:v1.0.0
ghcr.io/afewell-hh/demon-runtime:sha-{git-sha}

ghcr.io/afewell-hh/demon-engine:latest
ghcr.io/afewell-hh/demon-engine:v1.0.0
ghcr.io/afewell-hh/demon-engine:sha-{git-sha}
```

#### 2.3 Versioning Strategy
- **latest**: Always points to main branch HEAD
- **semver**: Manual tags for releases (v1.0.0, v1.1.0, etc.)
- **sha-{git-sha}**: Immutable build identifier for debugging

### Phase 3: K8s Integration (Week 2)

#### 3.1 Update K8s Manifests
Revert placeholder changes in:
- `demonctl/resources/k8s/operate-ui.yaml`
- `demonctl/resources/k8s/runtime.yaml`
- `demonctl/resources/k8s/engine.yaml`

**Changes:**
- Replace `nginx:alpine` with proper GHCR image references
- Restore health checks with correct endpoints
- Add image pull policy configuration
- Configure resource limits/requests

#### 3.2 Health Check Enhancement
Update `scripts/tests/smoke-k8s-bootstrap.sh`:
- Restore HTTP endpoint health checks
- Add retry logic for container startup
- Validate that actual Demon services are responding
- Maintain backwards compatibility during transition

### Phase 4: CI Integration (Week 2-3)

#### 4.1 Integration Points
**Existing workflows to update:**
- `bootstrapper-smoke.yml`: Use freshly built images for validation
- `ci.yml`: Add Docker build validation step

**New workflow dependencies:**
- Docker build must complete before smoke tests
- Image availability validation before K8s deployment

**Progress:**
- ✅ Docker build workflow now produces a reusable `docker-image-digests.json` artifact with component → digest mappings (exposed via workflow outputs).
- ✅ `ci.yml` invokes the docker-build workflow via `needs` and exports immutable GHCR digests to the dry-run smoke job.
- ✅ Nightly smoke workflow resolves the latest successful docker-build run on `main`, downloads the digest artifact with `gh` API access, and fails fast when artifacts expire (documented fallback to `:main`).
- 🔄 Follow-up: monitor nightly run after the first scheduled execution to ensure clusters pull digests without relying on local image import.

#### 4.2 Rollback Strategy
**Immediate rollback capability:**
- Keep placeholder manifests in a separate branch
- Environment variable to toggle between placeholder and real images
- Quick revert process documented

## Technical Implementation Details

### Dockerfile Template Structure
```dockerfile
# Multi-stage build for Rust components
FROM rust:1.75-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin {component}

FROM alpine:3.18
RUN adduser -D -s /bin/sh appuser
COPY --from=builder /app/target/release/{component} /usr/local/bin/
USER appuser
EXPOSE {port}
CMD ["{component}"]
```

### GitHub Actions Workflow Structure
```yaml
name: Build and Push Docker Images
on:
  push:
    branches: [main]
  pull_request:
    paths: ['**/Dockerfile', 'operate-ui/**', 'runtime/**', 'engine/**']

jobs:
  build:
    strategy:
      matrix:
        component: [operate-ui, runtime, engine]
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v5
        with:
          context: ./${{ matrix.component }}
          push: ${{ github.ref == 'refs/heads/main' }}
          tags: |
            ghcr.io/afewell-hh/demon-${{ matrix.component }}:latest
            ghcr.io/afewell-hh/demon-${{ matrix.component }}:sha-${{ github.sha }}
```

## Migration Steps

### Step 1: Infrastructure Setup
1. Create Dockerfiles (operate-ui, runtime, engine)
2. Create docker-build.yml workflow
3. Test Docker builds locally
4. Validate GHCR authentication

### Step 2: Image Publishing
1. Merge Docker infrastructure to main
2. Verify images are published to GHCR
3. Test image pulls from GHCR
4. Validate image functionality

### Step 3: K8s Integration
1. Update K8s manifests with real image references
2. Update smoke test health checks
3. Run full integration test
4. Monitor nightly validation

### Step 4: Cleanup
1. Remove placeholder TODO comments
2. Update documentation
3. Archive GHCR_FIX_SUMMARY.md
4. Update README with Docker instructions

## Risk Assessment

### Low Risk
- **Docker build failures**: Can be tested locally before merge
- **GHCR authentication**: Uses standard GitHub token approach
- **Rollback capability**: Placeholder images remain available

### Medium Risk
- **Component runtime issues**: Need thorough testing of actual services
- **Health check compatibility**: Must validate endpoint availability
- **CI pipeline disruption**: Could temporarily break nightly validation

### Mitigation Strategies
- **Staged rollout**: Test with individual components first
- **Parallel validation**: Keep both placeholder and real image paths during transition
- **Quick rollback**: Documented process to revert to placeholders
- **Pre-merge validation**: Require successful integration test before merge

## Success Criteria

### Completion Checklist
- [ ] Dockerfiles created for all 3 components
- [ ] Docker build workflow implemented and tested
- [ ] Images successfully published to GHCR
- [ ] K8s manifests updated with real image references
- [ ] Health checks restored and working
- [ ] Nightly smoke test passing with real images
- [ ] Documentation updated
- [ ] Placeholder references removed

### Performance Targets
- **Build time**: < 10 minutes per component
- **Image size**: < 100MB per component (optimized)
- **CI reliability**: 95%+ success rate for Docker builds
- **Startup time**: < 30 seconds for pod ready state

## Timeline

### Week 1: Foundation
- Days 1-2: Create Dockerfiles and test locally
- Days 3-4: Implement Docker build workflow
- Day 5: GHCR integration and initial image publishing

### Week 2: Integration
- Days 1-2: Update K8s manifests and health checks
- Days 3-4: Full integration testing and validation
- Day 5: Production deployment and monitoring

### Week 3: Optimization & Cleanup
- Days 1-2: Performance optimization and security hardening
- Days 3-4: Documentation updates and cleanup
- Day 5: Final validation and project closure

## Dependencies

### External
- **GHCR availability**: GitHub Container Registry service
- **Docker Hub**: For base images (rust, alpine)
- **Rust toolchain**: Stable nightly version compatibility

### Internal
- **Cargo workspace**: All components must build successfully
- **Current CI pipeline**: Must not disrupt existing validation
- **K8s manifests**: Must maintain deployment compatibility

## Future Considerations

### Enhancements
- **Multi-architecture builds**: ARM64 support for Apple Silicon
- **Security scanning**: Integrate vulnerability scanning tools
- **Image optimization**: Distroless base images for smaller attack surface
- **Caching strategy**: Registry-based layer caching for faster builds

### Monitoring
- **Image size tracking**: Monitor and alert on image bloat
- **Build performance**: Track build time trends
- **Security alerts**: Automated vulnerability notifications
- **Usage metrics**: Monitor image pull statistics

---

**Next Action**: Create epic/story issue for implementing this Docker pipeline plan and assign to appropriate team member for execution.
