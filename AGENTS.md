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

---

## Project Management & Tracking (MVP)
- **Single source**: keep MVP scope in `docs/mvp/01-mvp-contract.md` (problem, personas, **M0 must-haves** with acceptance criteria, release criteria).  
  - **Progress** = checked M0 items / total M0 items × 100. Update checkboxes as capabilities land.
- **Epics list**: `docs/mvp/02-epics.md` — table of epics → link issues/PRs.
- **Issues**: open a *Story* issue per M0 capability; label with `story`, priority (`p0`/`p1`), and area (`area:backend`/`area:frontend`), milestone `MVP-Alpha`/`MVP-Beta`.
- **Linkage**: PRs must reference their story (`Fixes #NNN`) and the epic (link in PR body).
- **Weekly status**: open an issue titled `MVP Status — YYYY-MM-DD` summarizing % complete, risks, next steps.

## Labels, Milestones & Projects
- **Labels**: `epic`, `story`, `p0`, `p1`, `area:backend`, `area:frontend`, `needs-spec`, `triage-comment`, `enforce-review-replies`.
- **Saved labels behavior**:
  - `triage-comment` → posts/updates a sticky PR triage summary (visibility only).
  - `enforce-review-replies` → runs replies-guard; fails on unresolved **no-reply** threads; docs-only PRs self-skip.
- **Milestones**: `MVP-Alpha`, `MVP-Beta`. Assign all MVP issues.
- **Project**: track stories in GitHub Project “Demon MVP”; set fields (Status, Area, Priority, Target Release).

## Review Hygiene & Replies Policy
- **Every review comment MUST receive an explicit author reply before merge.**  
  Use one of: “Fixed in `<short-sha>`”, “Clarified: …”, “Won’t fix because …”.
- **No open threads at merge**: conversation resolution is enabled on `main`. Resolve after replying.
- **Guard**: `review-threads-guard (PR) / guard` enforces author replies on unresolved threads (label-gated or always-run/self-skip per workflow).  
  - Docs-only PRs are exempt (guard self-skips); still reply when feedback is substantive.
- **Triage tools**:
  - `make audit-triage` → generates triage report; apply `triage-comment` to post sticky summary.
  - `./audit-pr-threads.sh` / `./audit-pr-triage-md.sh` — deeper audits when needed.

## Branch Protection & Required Checks (**DO NOT RENAME**)
- Required contexts on `main` (names **must** match exactly, including en-dashes “—”):
  - `Bootstrapper bundles — verify (offline, signature ok)`
  - `Bootstrapper bundles — negative verify (tamper ⇒ failed)`
  - `contracts-validate`
  - `review-lock-guard`
  - `review-threads-guard (PR) / guard`
- **Review-lock**: PR body MUST contain the current HEAD SHA. Update on every push.  
  Example: `gh pr view $PR --json headRefOid -q .headRefOid` → put in body as `Review-lock: <sha>`.
- **Conversation resolution**: on; **Include administrators**: on. Do not bypass protections.
- **Governance snapshot**: when protection settings change, commit `.github/snapshots/branch-protection-YYYY-MM-DD.json`.

## PR Lifecycle — Agent Checklist (follow in order)
- ✅ Small, focused diff; story linked (`Fixes #…`); correct labels & milestone.
- ✅ `make fmt && make lint && make test` pass locally.
- ✅ Contracts/goldens updated if events changed.
- ✅ UI/Playwright: wait for HTTP 200 from `/api/runs`; tests resilient to `{runs:[]}`, array response, or `{error}`.
- ✅ PR template checklists ticked; screenshots/logs attached when relevant.
- ✅ **Review-lock** updated to HEAD; **every review comment replied**; threads resolved.
- ✅ Required CI checks green (4 contexts above). If guard fails: reply in-thread, re-run, resolve.

## CI, Tokens & Secrets
- **Local GH auth**: `gh` must be authenticated as `afewell-hh`. `.env` may export `GH_TOKEN` for CLI use (do not commit).
- **Admin/tokenized calls in CI**:
  - Use `PROJECT_ADMIN_TOKEN` for Project V2 automation (Project Backfill workflow); falls back to `ADMINTOKEN_DEMON_AFEWELLHH` if absent.
  - Use `PROTECTION_TOKEN` **only** in workflows that gate admin endpoints; guard with `if: env.PROTECTION_TOKEN != ''` and request minimal `permissions`.
  - Never print tokens; never echo secrets to logs.
- **Cargo/security**:
  - `cargo-deny` step may be `continue-on-error: true` (non-protected signal).
  - `cargo-audit` pin/version as needed to toolchain; do not make it a required status unless agreed.
- **Flake handling**:
  - Playwright: retry with backoff; ensure server readiness (HTTP 200) before running tests.

## Ops Quick-Refs (for agents)
- **Update Review-lock** (append to body):
  - `PR=NNN; HEAD=$(gh pr view $PR --json headRefOid -q .headRefOid); gh pr edit $PR -b "$(gh pr view $PR -q .body)\n\nReview-lock: $HEAD"`
- **Post triage sticky**:
  - `make audit-triage && gh pr edit <PR> --add-label triage-comment`
- **Run replies-guard** (label-gated mode):
  - `gh pr edit <PR> --add-label enforce-review-replies`
- **Snapshot protections**:
  - `SNAP=.github/snapshots/branch-protection-$(date -u +%F).json; gh api repos/:owner/:repo/branches/main/protection > "$SNAP"; git add "$SNAP" && git commit -m "governance: snapshot branch protection ($(date -u +%F))" && git push`

## Visualization & Agent Flow Quick-Refs (Sprint D)
- **Enable Canvas UI** (interactive DAG viewer):
  - `export OPERATE_UI_FLAGS=canvas-ui && cargo run -p operate-ui`
  - Navigate to `http://localhost:3030/canvas` for force-directed ritual visualization
  - Docs: [`docs/canvas-ui.md`](docs/canvas-ui.md)
- **Enable Contracts Browser** (schema registry explorer):
  - `export OPERATE_UI_FLAGS=contracts-browser && export SCHEMA_REGISTRY_URL=http://localhost:8080 && cargo run -p operate-ui`
  - Navigate to `http://localhost:3000/ui/contracts` for contract search/browse
  - Docs: [`docs/operate-ui/README.md#contracts-browser`](docs/operate-ui/README.md#contracts-browser)
- **Export ritual as flow manifest**:
  - `cargo run -p demonctl -- flow export --ritual echo --output my-flow.json`
  - Supports JSON/YAML output via file extension
  - Docs: [`docs/agent-flows.md`](docs/agent-flows.md)
- **Import/submit flow manifest**:
  - Validate: `cargo run -p demonctl -- flow import --file my-flow.json --dry-run`
  - Submit: `export DEMONCTL_JWT="your-token" && cargo run -p demonctl -- flow import --file my-flow.json --api-url http://localhost:3000`
  - Requires `agent-flows` feature flag and JWT with `flows:write` scope
  - Docs: [`docs/agent-api.md`](docs/agent-api.md)
- **List contracts for flow authoring**:
  - `curl -H "Authorization: Bearer $JWT_TOKEN" -H "X-Demon-API-Version: v1" http://localhost:3000/api/contracts`

## Source Control & Secrets Hygiene
- No secrets in repo. `.env` stays local; use GitHub Secrets for CI.
- Do not force-push to protected branches. Do not rename required jobs or checks.
- Admin/bypass merges are prohibited unless a documented emergency playbook is invoked and governance snapshots are committed before/after.

## Documentation Maintenance
- Keep `README.md` Quickstart and “How we use labels” current.
- Update `docs/process/` HOWTOs when workflows/labels/guards change.
- When closing a story, check the box in `docs/mvp/01-mvp-contract.md` and link the merged PR for traceability.
