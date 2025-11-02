# Contributing

This repo uses two labels to keep review feedback visible and ensure responsiveness without blocking healthy discussion.

- `triage-comment` — Posts/updates a sticky triage summary on the PR with deep links to unresolved review threads. Visibility only; does not affect status checks.
- `enforce-review-replies` — Runs the `review-threads-guard (PR)` check. The PR fails if any unresolved review thread has no reply from the PR author. Pairs well with GitHub's "Require conversation resolution".

## Security Expectations

All contributors must follow security best practices. See [`docs/security.md`](docs/security.md) for complete guidelines.

### Quick Security Checklist

Before submitting a PR:

- [ ] **Format & Lint**: Run `make fmt && make lint` (CI will deny warnings)
- [ ] **Security Audits**: `cargo audit --deny warnings` passes locally
- [ ] **No Secrets**: No credentials, tokens, or sensitive data in code/config
- [ ] **Container Digests**: All Docker base images pinned to SHA-256 digests
- [ ] **Sandbox Flags**: No changes to container-exec security flags without approval
- [ ] **Tests Pass**: All workspace tests pass (`make test`)

### RUSTSEC Advisory Policy

- No new security advisories introduced by dependency updates
- Suppressions in `.cargo/audit.toml` require justification, expiry date, and issue link
- CI fails on any RUSTSEC warnings (see `cargo-audit` job in workflows)

### Container Image Security

- **Never** use tag-only references (e.g., `rust:alpine`)
- **Always** pin to specific digests (e.g., `rust:alpine@sha256:abc123...`)
- Update digests deliberately with testing
- See [`docs/security.md#container-image-security`](docs/security.md#container-image-security) for update procedures

### Sandbox Enforcement

The following container-exec flags are **mandatory** and verified by CI:
- `--network=none` (no network access)
- `--read-only` (read-only root filesystem)
- `--security-opt no-new-privileges` (no privilege escalation)
- `--tmpfs /tmp` with `noexec,nosuid,nodev` (restricted writable temp)

These flags **may not be removed** without security team approval.

## Development Workflow

Handy commands
- `make audit-triage` — Generate a repo triage Markdown report for the last N PRs (`COUNT=60 make audit-triage`).
- `make audit-triage-issue` — Generate today's report and open an issue with it attached.
- `make fmt` — Format code with rustfmt
- `make lint` — Run clippy with warnings denied
- `make test` — Run all workspace tests

Notes
- Both review triage workflows can also be triggered manually via "Run workflow".
- Administrators are encouraged to enable "Require conversation resolution" (include administrators) on `main` and mark `review-threads-guard (PR) / guard` as required once stable.
