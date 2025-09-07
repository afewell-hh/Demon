# ADR-0002 — Rust Toolchain Policy

Status: Accepted

Date: 2025-09-07

## Decision

Pin the workspace to Rust stable 1.82.0 for reproducible builds and CI determinism.

Nightly is allowed only behind a targeted exception ADR that states: rationale, crates/features requiring nightly, owner, rollback plan, and an expiry date.

## Consequences

- CI and local dev use the same compiler; fewer "works on my machine" failures.
- If nightly is ever needed, changes are isolated and time-boxed with a rollback path.
