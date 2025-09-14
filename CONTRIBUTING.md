# Contributing

This repo uses two labels to keep review feedback visible and ensure responsiveness without blocking healthy discussion.

- `triage-comment` — Posts/updates a sticky triage summary on the PR with deep links to unresolved review threads. Visibility only; does not affect status checks.
- `enforce-review-replies` — Runs the `review-threads-guard (PR)` check. The PR fails if any unresolved review thread has no reply from the PR author. Pairs well with GitHub’s “Require conversation resolution”.

Handy commands
- `make audit-triage` — Generate a repo triage Markdown report for the last N PRs (`COUNT=60 make audit-triage`).
- `make audit-triage-issue` — Generate today’s report and open an issue with it attached.

Notes
- Both review triage workflows can also be triggered manually via “Run workflow”.
- Administrators are encouraged to enable “Require conversation resolution” (include administrators) on `main` and mark `review-threads-guard (PR) / guard` as required once stable.
