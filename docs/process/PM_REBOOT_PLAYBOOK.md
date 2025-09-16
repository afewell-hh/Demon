# PM Reboot Playbook — Demon Project

Purpose: enable a fresh PM session (new context window) to regain full mastery quickly, drive coding agents effectively, and preserve governance. This guide is timeless and references durable repo assets you can read on day one.

## 1) Ground Truth — What To Read First
- Contract: docs/mvp/01-mvp-contract.md:1 — Source of truth for MVP capabilities and checkboxes.
- Epics: docs/mvp/02-epics.md:1 — Epics, links to issues/PRs, and prioritization.
- Branch protections (policy): docs/process/branch_protection_mvp.md:1 — Required checks, replies‑guard stance.
- Governance ops: .github/GOVERNANCE.md:1 — Audit, snapshot, CLI recipes.
- CI workflow: .github/workflows/ci.yml:415 — DO NOT RENAME sections; required job names.
- README quickstart: README.md:1 — Smoke instructions; Approvals/TTL notes.

## 2) Non‑Negotiables (Guardrails)
- Required checks on `main` (names must match exactly):
  - Bootstrapper bundles — verify (offline, signature ok)
  - Bootstrapper bundles — negative verify (tamper ⇒ failed)
  - review-lock-guard
- Replies‑guard: non‑required during MVP (still reply to substantive feedback).
- Review‑lock: PR body must end with `Review-lock: <40‑char HEAD SHA>`.
- Protection settings: conversation resolution enabled; include administrators; linear history; squash merges.
- Strict mode: `strict:false` (avoid “update branch” churn). Do not rename protected jobs.

## 3) Tokens & Secrets (Usage Patterns)
- Local `.env` (never commit values): `.env:1`
  - `GITHUB_TOKEN` — owner/admin; prefer for GH API/GraphQL; export as `GH_TOKEN`.
  - `REVIEWER_TOKEN` — collaborator context (fallback only when necessary).
  - `PROTECTION_TOKEN` — admin read‑only for audits.
- Actions Secrets (never print): repo settings
  - `ADMINTOKEN_DEMON_AFEWELLHH` — fine‑grained for Project V2 automation.
  - `PROTECTION_TOKEN` — protection audit workflow.
  - `REVIEWER_TOKEN` — collaborator automation if needed.
- Pattern: `export GH_TOKEN="$GITHUB_TOKEN"` for CLI; never echo tokens.

## 4) CI & Workflow Invariants
- Job names are frozen in `.github/workflows/ci.yml:415` (DO NOT RENAME blocks). Keep verify/negative‑verify intact.
- If verify job skips due to DAG, decouple via `needs:` safely without renaming the job.
- Cargo‑deny philosophy: keep policy strict; if flake, add the narrowest SPDX or time‑boxed advisory ignore with TODO.
- Never broaden license allowlists casually; document exceptions.

## 5) Project Management System
- Issues & labels: `story`, `p0`/`p1`, `area:backend`/`area:frontend`, milestone `MVP‑Alpha/Beta`.
- Project board (V2): https://github.com/users/afewell-hh/projects/1 (configure views Backlog/Area/Target Release).
- Automation:
  - Auto‑add: .github/workflows/project-auto-add.yml:1 — adds `story` issues, sets defaults (handles single‑select/text).
  - Backfill: .github/workflows/project-backfill.yml:1 — manual `workflow_dispatch` with `issue_numbers`.
- GraphQL quick‑refs (Project V2): see docs/process/MVP.md:1 and .github/GOVERNANCE.md:29 for API usage patterns.

## 6) First Hour In A New Session (Checklist)
1) Read the files in Section 1 and skim CI workflow jobs.
2) Validate protections via governance:
   - Snapshot: .github/snapshots/ — most recent shows exactly 3 required checks, `strict:false`.
   - Run protection audit and check logs (see Section 8).
3) Inspect open PRs and Issues:
   - Ensure Review‑lock lines present; required checks green or being worked.
4) Smoke local build:
   - Commands: `make dev`; `cargo run -p demonctl -- run examples/rituals/echo.yaml --jetstream`.
5) Update weekly status issue (e.g., `#73`) with % complete and next steps.

## 7) Coding Agent Dispatch Template (Per Task)
- Context links: contract, epic, issue, and board view.
- Guardrails: required job names; Review‑lock last line; replies‑guard non‑required but reply to all comments.
- Tokens to use: `GH_TOKEN=$GITHUB_TOKEN` (owner), use Actions secrets for workflows.
- Deliverables:
  - Minimal diff; tests; docs; updated fixtures/schemas if events changed.
  - PR body: `Fixes #NNN` + acceptance bullets + `Review-lock: <HEAD>`; auto‑merge armed.
- Acceptance: enumerate concrete, testable checks; include smoke commands.

## 8) Ops Quick‑Refs (Copy/Paste)
- Update Review‑lock:
  - ``PR=NNN; HEAD=$(gh pr view $PR --json headRefOid -q .headRefOid); gh pr edit $PR -b "$(gh pr view $PR -q .body)\n\nReview-lock: $HEAD"``
- Run protection audit and fetch logs:
  - `gh workflow run protection-audit.yml`
  - ``gh run view "$(gh run list --workflow protection-audit.yml --limit 1 --json databaseId -q '.[0].databaseId')" --log``
- Snapshot branch protections:
  - ``SNAP=.github/snapshots/branch-protection-$(date -u +%F).json; gh api repos/:owner/:repo/branches/main/protection > "$SNAP"; git add "$SNAP" && git commit -m "governance: snapshot branch protection ($(date -u +%F))" && git push``
- Backfill project items (workflow):
  - `gh workflow run project-backfill.yml -f issue_numbers="56 57 58 59 60 61 62 63"`
- Add issue to Project V2 (GraphQL): see docs/process/MVP.md:1 (GraphQL examples included).

## 9) Typical Sprint Pattern (Thin Slices)
- Plan: create/confirm story issue with acceptance, labels, milestone; add to board.
- Implement: minimal, focused diffs; keep DO NOT RENAME invariants; instrument with `tracing`.
- Validate: `make fmt && make lint && make test` locally; add specs under `*_spec.rs` with Given/When/Then.
- Document: README changes (Quickstart/API), ADRs for architecture shifts, schemas/fixtures when events change.
- Govern: Review‑lock discipline; re‑run audit; snapshot after merges; update weekly status and project board.

## 10) Troubleshooting Playbooks
- CI red — verify skipped:
  - Check `.github/workflows/ci.yml` DAG for `needs:`; decouple verify without renaming job id/name.
- Cargo‑deny failure:
  - Pin tool version if needed; add narrow SPDX or time‑boxed advisory ignore in `deny.toml:1` with TODO.
- Protection audit failure:
  - Read assertion; align CI job names; verify token scopes; re‑run and snapshot.
- NATS/JetStream flakes:
  - Ensure `make dev` started; confirm stream info; add backoff/logging for SSE and consumers.

## 11) Key Paths (Quick Index)
- Contract: docs/mvp/01-mvp-contract.md:1
- Epics: docs/mvp/02-epics.md:1
- MVP one‑pager: docs/process/MVP.md:1
- PM reboot (this): docs/process/PM_REBOOT_PLAYBOOK.md:1
- Governance: .github/GOVERNANCE.md:1
- CI workflow: .github/workflows/ci.yml:415
- Protection audit: .github/workflows/protection-audit.yml:1
- Project auto‑add: .github/workflows/project-auto-add.yml:1
- Project backfill: .github/workflows/project-backfill.yml:1
- Snapshots: .github/snapshots/:1

## 12) Safety & Hygiene
- Never echo secrets/tokens; scope PATs minimally.
- Do not modify or rename protected job names or required contexts.
- Keep diffs small and reversible; add ADRs for architectural changes.
- Always reply to every review comment; resolve conversations after replying.

---
Use this playbook to bootstrap any new PM session and to brief fresh coding agents on each dispatch. It centers on invariants (guards), durable references (files/links), and minimal, testable slices to maintain velocity and safety.

