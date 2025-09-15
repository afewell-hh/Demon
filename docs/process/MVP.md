# MVP One-Pager — Scope, Protections, Merge Flow

This is the concise source of truth for MVP scope, protections, merge discipline, and quick commands. It complements:

- Contract: docs/mvp/01-mvp-contract.md
- Branch protections (MVP): docs/process/branch_protection_mvp.md
- Governance ops: .github/GOVERNANCE.md

## Scope Freeze (M0)
- Must-haves live in docs/mvp/01-mvp-contract.md — progress = checked M0 items / total M0 items × 100.
- Any scope change requires updating the contract and epics: docs/mvp/02-epics.md.

## Protections (MVP)
- Required checks on `main` (names must match exactly):
  - Bootstrapper bundles — verify (offline, signature ok)
  - Bootstrapper bundles — negative verify (tamper ⇒ failed)
  - review-lock-guard
- Non-required but running: review-threads-guard (PR) / guard
- Conversation resolution enabled; Include administrators enabled; linear history; squash merges only.

## Merge Flow
1) Add Review-lock line to PR body (last line): `Review-lock: <40-char HEAD SHA>`
2) `gh pr merge <num> --squash --delete-branch --auto` once green and approved.
3) Reply to every review comment explicitly; resolve after replying.
4) Keep required job names frozen. If CI changes, update workflows without renaming required contexts.

## Project Tracking — “Demon MVP”
- Board: https://github.com/users/afewell-hh/projects/1
- Stories are GitHub Issues for each M0 capability; label with: `story`, priority (`p0`/`p1`), area (`area:backend`/`area:frontend`), milestone (`MVP-Alpha`/`MVP-Beta`).
- Fields per item: Status, Area, Priority, Target Release.

### GraphQL quick refs (Project V2)
Fetch project ID:
```bash
gh api graphql -f query='query($login:String!){user(login:$login){projectV2(number:1){id}}}' -f login='afewell-hh' -q .data.user.projectV2.id
```
Add each issue by URL (node ID):
```bash
# Get issue node ID
gh api repos/:owner/:repo/issues/56 -q .node_id
# Add to project
gh api graphql -f query='mutation($proj:ID!,$item:ID!){addProjectV2ItemById(input:{projectId:$proj,contentId:$item}){item{id}}}' \
  -F proj="$PROJ" -F item="$ISSUE_NODE"
```
List fields to get IDs:
```bash
gh api graphql -F proj="$PROJ" -f query='query($proj:ID!){node(id:$proj){... on ProjectV2{fields(first:50){nodes{id name dataType}}}}}'
```
Set single-select values (Status/Area/Priority/Target Release):
```bash
gh api graphql -F proj="$PROJ" -F item="$ITEM" -F field="$FIELD" -F option="$OPTION" -f query='mutation($proj:ID!,$item:ID!,$field:ID!,$option:String!){updateProjectV2ItemFieldValue(input:{projectId:$proj,itemId:$item,fieldId:$field,value:{singleSelectOptionId:$option}}){projectV2Item{id}}}'
```

## Ops Quick-Refs
- Update Review-lock in PR body:
```bash
PR=NNN; HEAD=$(gh pr view $PR --json headRefOid -q .headRefOid); gh pr edit $PR -b "$(gh pr view $PR -q .body)\n\nReview-lock: $HEAD"
```
- Post triage sticky summary:
```bash
make audit-triage && gh pr edit <PR> --add-label triage-comment
```
- Enforce replies (label-gated mode):
```bash
gh pr edit <PR> --add-label enforce-review-replies
```
- Snapshot current branch protections:
```bash
SNAP=.github/snapshots/branch-protection-$(date -u +%F).json; gh api repos/:owner/:repo/branches/main/protection > "$SNAP"; git add "$SNAP" && git commit -m "governance: snapshot branch protection ($(date -u +%F))" && git push
```

## CI Notes
- `cargo-audit` job uses retry/pin strategy; if the installer flakes or a new advisory lands, open a focused PR to pin or allowlist with expiry and link a follow-up issue.
- Do not change required job names; keep DO NOT RENAME markers in workflows intact.

## Weekly Status
- Open an issue titled: `MVP Status — YYYY-MM-DD` with:
  - % complete = checked M0 / total M0 × 100
  - Risks and blockers
  - Next steps for the week

