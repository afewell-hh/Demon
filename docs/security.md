# Security Guidelines

This document outlines security practices, policies, and expectations for contributors to the Demon project.

## Table of Contents

- [Dependency Security](#dependency-security)
- [Container Image Security](#container-image-security)
- [Sandbox Enforcement](#sandbox-enforcement)
- [Secure Build Practices](#secure-build-practices)
- [Reporting Security Issues](#reporting-security-issues)

## Dependency Security

### RUSTSEC Advisory Handling

We use `cargo-audit` to scan for known security advisories in our dependency tree.

#### Policy

- **No suppressions by default**: All RUSTSEC advisories must be addressed.
- **Temporary suppressions**: May be allowed only when:
  1. The vulnerability does not affect our use case (documented with clear justification)
  2. No fix is available from upstream
  3. An expiry date is set (typically ≤ 12 months)
  4. Tracked in a GitHub issue

#### Configuration

RUSTSEC suppressions are configured in `.cargo/audit.toml`. Each suppression must include:
```toml
[advisories]
ignore = [
    # RUSTSEC-YYYY-NNNN (crate: brief description).
    # Justification: Explain why this advisory does not affect Demon's use case.
    # Expiry: Remove by YYYY-MM-DD or when [condition].
    # Reference: https://github.com/afewell-hh/Demon/issues/NNN
    "RUSTSEC-YYYY-NNNN",
]
```

#### Running Audits

```bash
# Install cargo-audit
cargo install cargo-audit --locked

# Run audit (fails on warnings)
cargo audit --deny warnings

# CI automatically runs cargo-audit on every PR
```

### Dependency Updates

- Keep dependencies reasonably up-to-date
- Review changelogs and breaking changes before updating
- Test thoroughly after dependency updates
- Pin exact versions in `Cargo.lock` (committed to repository)

## Container Image Security

### Digest Pinning Policy

**All container base images MUST be pinned to specific SHA-256 digests** to prevent supply chain attacks.

#### Policy

- **Never use tag-only references** (e.g., `rust:alpine`, `distroless:latest`)
- **Always pin to digests** (e.g., `rust:alpine@sha256:abc123...`)
- **Update digests deliberately**: When updating base images, explicitly pull new digest and test

#### Current Pinned Images

| Image | Digest (as of 2025-11-02) |
|-------|---------------------------|
| `rust:alpine` | `sha256:a3e3d30122c08c0ed85dcd8867d956f066be23c32ed67a0453bc04ce478ad69b` |
| `gcr.io/distroless/static-debian12` | `sha256:87bce11be0af225e4ca761c40babb06d6d559f5767fbf7dc3c47f0f1a466b92c` |

#### Updating Base Images

```bash
# Pull image and get digest
docker pull rust:alpine
docker inspect rust:alpine --format='{{index .RepoDigests 0}}'

# Example output: rust:alpine@sha256:a3e3d30122c08...

# Update Dockerfiles with new digest
sed -i 's|FROM rust:alpine@sha256:.*|FROM rust:alpine@sha256:NEW_DIGEST|g' */Dockerfile

# Test builds
docker build -f runtime/Dockerfile .
docker build -f operate-ui/Dockerfile .
docker build -f engine/Dockerfile .
```

## Sandbox Enforcement

### Container-Exec Security Flags

The `container-exec` capsule enforces strict sandboxing on all executed containers. The following flags are **mandatory** and verified by CI:

#### Required Flags

1. **`--network=none`**: Disables all network access
2. **`--read-only`**: Makes root filesystem read-only
3. **`--security-opt no-new-privileges`**: Prevents privilege escalation
4. **`--tmpfs /tmp`**: Provides writable `/tmp` with restrictions:
   - `rw`: Read-write access
   - `noexec`: Cannot execute binaries from /tmp
   - `nosuid`: Ignores suid/sgid bits
   - `nodev`: No device file creation
   - `size=67108864`: 64 MB limit

#### Verification

CI automatically verifies these flags remain in `capsules/container-exec/src/lib.rs`:

```bash
# Manually verify sandbox flags
./scripts/verify-sandbox-flags.sh
```

#### Exception Policy

Sandbox flags **may not be removed or weakened** without:
1. Explicit approval from security team
2. Documentation of security implications
3. Alternative mitigations in place

### Resource Limits

Optional resource limits can be configured via environment variables:
- `DEMON_CONTAINER_CPUS`: CPU limit (e.g., `0.5`)
- `DEMON_CONTAINER_MEMORY`: Memory limit (e.g., `256m`)
- `DEMON_CONTAINER_PIDS_LIMIT`: Process limit (e.g., `128`)

## Secure Build Practices

### Local Development

```bash
# Format code (required before commit)
make fmt

# Lint with warnings denied
make lint

# Run security audits
cargo audit --deny warnings
cargo deny check --all-features

# Run tests
make test
```

### CI/CD Security Checks

Every PR must pass:
1. **Format check**: `cargo fmt --all -- --check`
2. **Lint check**: `cargo clippy --workspace --all-targets -- -D warnings`
3. **Cargo audit**: Security advisory scan
4. **Cargo deny**: License and ban checks
5. **Sandbox verification**: Container-exec flag verification
6. **Tests**: Full workspace test suite

### Secrets Management

- **Never commit secrets** to the repository
- Use `.env` for local secrets (gitignored)
- Use GitHub Secrets for CI/CD secrets
- Tokens should have minimal required permissions
- Rotate secrets regularly

### Code Review Requirements

Security-sensitive changes require:
- Review from at least one maintainer
- All review comments addressed with explicit replies
- Passing CI checks
- Updated documentation (if applicable)

## Reporting Security Issues

### Reporting Process

**Do not open public GitHub issues for security vulnerabilities.**

Instead:
1. Email: [Add security contact email]
2. Use GitHub Security Advisories (if enabled)
3. Provide:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 1 week
- **Fix timeline**: Depends on severity (communicated in assessment)
- **Public disclosure**: Coordinated after fix is released

### Severity Levels

| Level | Description | Response Time |
|-------|-------------|---------------|
| **Critical** | Remote code execution, privilege escalation | 24-48 hours |
| **High** | Data exposure, authentication bypass | 1 week |
| **Medium** | Denial of service, information disclosure | 2 weeks |
| **Low** | Minor security improvements | Next release cycle |

## Security Checklist for Contributors

Before submitting a PR that touches security-sensitive areas:

- [ ] No new RUSTSEC advisories introduced
- [ ] Container images are digest-pinned
- [ ] Sandbox flags unchanged (or approved exception)
- [ ] No secrets in code or config files
- [ ] Input validation on all external inputs
- [ ] Error messages don't leak sensitive information
- [ ] Tests cover security-relevant edge cases
- [ ] Documentation updated (if security behavior changed)

## References

- [Cargo Audit Documentation](https://github.com/RustSec/rustsec/tree/main/cargo-audit)
- [Cargo Deny Documentation](https://github.com/EmbarkStudios/cargo-deny)
- [Docker Security Best Practices](https://docs.docker.com/engine/security/)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)

## Changelog

| Date | Change | Author |
|------|--------|--------|
| 2025-11-02 | Initial security documentation | System |
