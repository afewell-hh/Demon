# Demon — Preview Alpha (Index)

**Tag:** `preview-alpha-1`  •  **SHA:** `27e36b21136e`

This folder contains the one-stop kit to run and narrate the Alpha preview on a clean machine.

## Docs

- **Runbook:** [`runbook.md`](./runbook.md) — 10-minute flow, envs, success criteria, and verification snippets.
- **Deck (5 slides):** [`deck.md`](./deck.md) — talk track outline for a short client readout.
- **Presenter Script (≈60s):** [`presenter_script.md`](./presenter_script.md) — concise narration.
- **Dry-Run Checklist:** [`dry_run_checklist.md`](./dry_run_checklist.md) — clean VM steps & troubleshooting.

## Screenshots

- Runs list — [`screenshots/runs_list.png`](./screenshots/runs_list.png)
- Approvals (granted) — [`screenshots/approval_granted.png`](./screenshots/approval_granted.png)
- TTL auto-deny (expired) — [`screenshots/ttl_expired.png`](./screenshots/ttl_expired.png)

## Quick Start

1. Start NATS (JetStream): `make dev`
2. Start Operate UI: `cargo run -p operate-ui`
3. Start TTL worker: `cargo run -p engine --bin demon-ttl-worker`
4. Seed runs: `./examples/seed/seed_preview.sh`

> If `4222` is busy, set `NATS_PORT` and re-run. The seeder and UI honor `RITUAL_STREAM_NAME=RITUAL_EVENTS`.

## Success Criteria (fast checks)

- `/api/runs` returns an array (≥1).
- **Run A:** `policy.decision:v1` allow → deny (quota; camelCase `quota`).
- **Run B:** `approval.requested:v1` → `approval.granted:v1` (REST).
- **Run C:** single `approval.denied:v1` with `reason:"expired"` (TTL worker).


Test plan (what to verify before merge)

Open the new README in the PR preview—ensure all links are clickable and resolve within the repo.

Thumbnails render in the GitHub UI for each PNG.

No changes to code; build/lint remain green:

cargo build --locked --workspace

cargo fmt -- --check

cargo clippy -- -D warnings

Commit guidance

1 commit, ≤50 LOC:

docs(preview): add Preview Alpha index (README)

GitHub workflow (use gh; include review ping)

git checkout -b docs/preview-alpha-index
# add docs/preview/alpha/README.md with the content above
git add docs/preview/alpha/README.md
git commit -m "docs(preview): add Preview Alpha index (README)"
git push -u origin docs/preview-alpha-index

gh pr create --title "docs(preview): add Preview Alpha index (README)" \
  --body "Docs-only; adds a one-stop index for the Alpha preview kit.\n- Links to runbook, deck, presenter script, dry-run checklist\n- Links to three screenshots\n- Includes tag/sha (preview-alpha-1 / 27e36b21136e)\n\nEvidence: build/fmt/clippy remain green." \
  --draft
gh pr comment --body "@codex review"
gh pr ready


Acceptance

PR merged (squash) with the single new file.

Links confirmed and rendering in GitHub.

Final PR comment includes the tag/sha for traceability (preview-alpha-1 / 27e36b21136e).
