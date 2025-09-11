# Contributing

## Review-lock
- Paste the current HEAD commit SHA (40 chars) into the PR body’s “Review-lock” field.
- If the review-lock-guard fails, update the PR body to the latest HEAD SHA and it will pass on re-run.

## Evidence
- Include build/format/lint evidence and any relevant logs (e.g., verify-only JSON lines) in a PR comment.

## Required checks
- Merges require the following checks to be green:
  - Bootstrapper bundles — verify (offline, signature ok)
  - Bootstrapper bundles — negative verify (tamper ⇒ failed)
  - review-lock-guard

## Code Owners
- Changes under `bootstrapper/`, `contracts/`, or `.github/workflows/ci.yml` will request a Code Owner review.

