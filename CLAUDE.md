# Repository Guidelines

## Project Structure & Module Organization
- Rust workspace (`Cargo.toml`) with crates: `engine/`, `runtime/`, `demonctl/`, `operate-ui/`, and sample capsule `capsules/echo/`.
- Contracts live in `contracts/` (`schemas/` JSON Schemas, `fixtures/` goldens, `wit/`).
- Docs and requests in `docs/` (see `process/` and `request/`).
- Examples under `examples/rituals/` (e.g., `echo.yaml`).
- Dev Docker files in `docker/dev/` (NATS JetStream profile).

## Build, Test, and Development Commands
- `make dev` — start NATS via Compose and build workspace.
- `make up` / `make down` — bring dev NATS up/down.
- `make build` — `cargo build --workspace`.
- `make test` — run all workspace tests.
- `make fmt` — format via rustfmt; `make lint` — clippy (warnings denied in CI).
- Quick smoke: `cargo run -p demonctl -- run examples/rituals/echo.yaml`.

## Coding Style & Naming Conventions
- Toolchain: Rust nightly (see `rust-toolchain.toml`); edition 2021.
- Format with `cargo fmt`; lint with `cargo clippy -- -D warnings`.
- Naming: crates `kebab-case` (e.g., `operate-ui`), modules/files `snake_case`, types `CamelCase`, constants `SCREAMING_SNAKE_CASE`.
- Keep functions small; prefer `anyhow::Result` and `thiserror` for explicit errors; instrument with `tracing`.

## Testing Guidelines
- Use Rust’s built-in test harness. Place integration tests in `crate/tests/` (e.g., `engine/tests/…`).
- Prefer `_spec.rs` filenames and Given/When/Then descriptions in test names.
- Validate contracts with schemas and update goldens in `contracts/fixtures/` when events change.
- Run locally: `cargo test --workspace --all-features -- --nocapture`.

## Commit & Pull Request Guidelines
- Small, focused commits (≈≤200 LOC). Clear, imperative subject; reference a REQUEST (e.g., `docs/request/REQUEST-M1A-*.md`).
- If a test is `#[ignore]`, include `Justify-Ignore:` in the commit message (see `docs/process/GIT_HOOKS.md`).
- PRs: use the template; link REQUEST, tick checklists (contracts, tests, docs), include screenshots/logs for UI/CLI when helpful; CI must be green.

## Security & Configuration Tips
- Dev NATS ports: `NATS_PORT=4222`, `NATS_MON_PORT=8222`. `.env` is gitignored.
- Never commit secrets or runtime data (`.demon/` is ignored). Prefer env vars and local overrides.

## Architecture Overview
- `engine` interprets rituals and emits events; `runtime` routes capsule calls; `demonctl` is the CLI; `operate-ui` serves read-only views.
