Title: Preview Alpha — Customer Preview Kit
Tag: preview-alpha-1 • SHA: 27e36b21136e

What’s in this preview

Operate UI (read-only) with runs list and run detail views

Event log & replay (JetStream) with durable timers

Wards policy decisions (allow/deny) and Approvals (requested/granted/denied)

TTL auto-deny for pending approvals (worker-backed)

Preview Kit (start here)

Index (one stop): docs/preview/alpha/README.md

Runbook (10-min demo): docs/preview/alpha/runbook.md

Presenter Script (≈60s): docs/preview/alpha/presenter_script.md

Dry-Run Checklist (clean VM): docs/preview/alpha/dry_run_checklist.md

Deck (5 slides): docs/preview/alpha/deck.md

Screenshots

Runs list: docs/preview/alpha/screenshots/runs_list.png

Approvals (granted): docs/preview/alpha/screenshots/approval_granted.png

TTL auto-deny (expired): docs/preview/alpha/screenshots/ttl_expired.png

Quick success criteria

/api/runs returns an array (≥1)

Run B shows approval.requested:v1 → approval.granted:v1

Run C shows exactly one approval.denied:v1 with reason:"expired"

UI pages render without template errors

Notes

Stream name defaults: RITUAL_STREAM_NAME=RITUAL_EVENTS

Approver allow-list required for REST grant: APPROVER_ALLOWLIST

TTL worker can be enabled with TTL_WORKER_ENABLED=1

Traceability

Tag: preview-alpha-1

SHA: 27e36b21136e

Commands

# 1) Add the release notes file
git checkout -b release/preview-alpha-1-notes
# (create docs/preview/alpha/RELEASE_NOTES_preview-alpha-1.md with the text above)
git add docs/preview/alpha/RELEASE_NOTES_preview-alpha-1.md
git commit -m "docs(release): add preview-alpha-1 release notes"
git push -u origin release/preview-alpha-1-notes

# 2) Open a small docs-only PR (optional for review)
gh pr create --title "docs(release): add preview-alpha-1 release notes" \
  --body "Docs-only. Adds the release body used for the GitHub Release." --draft
gh pr ready

# 3) After merge, publish the GitHub Release
gh release create preview-alpha-1 \
  -t "Preview Alpha — Customer Preview Kit" \
  -F docs/preview/alpha/RELEASE_NOTES_preview-alpha-1.md
