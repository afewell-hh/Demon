# Branch Protection — MVP Policy

This repository uses a minimal, reliable protection set during MVP to keep progress fast while preserving safety. We will revisit stricter gating after MVP when CI stability and cadence allow.

Required status checks on `main`:
- Bootstrapper bundles — verify (offline, signature ok)
- Bootstrapper bundles — negative verify (tamper ⇒ failed)
- contracts-validate
- review-lock-guard

Non‑required (still runs):
- review-threads-guard (PR) / guard
- Additional contract workflows beyond `contracts-validate` continue to run but are not required.

Other protections:
- Conversation resolution enabled (no open threads at merge).
- Enforce administrators enabled.
- Linear history enabled; squash merges only; branches deleted on merge.
- Auto‑merge enabled for approved, green PRs.

Workflow guidance:
- Always include `Review-lock: <PR HEAD SHA>` as the last line of the PR body. The `review-lock-guard` verifies it and reduces timing flakes.
- Prefer `--auto` merges: `gh pr merge <num> --squash --delete-branch --auto`.
- If checks lag, re‑queue via an empty commit or comment; avoid rebasing just to satisfy “update branch” (strict=false).

Post‑MVP plan:
- Stabilize replies guard emission on all PR events, then consider re‑adding it to the required checks.
