# Governance & Branch Protections

## Branch rules + required checks snapshot (main)

The following command snapshots branch protection for `main`:

```
gh api repos/:owner/:repo/branches/main/protection \
  -q '{required_status_checks:.required_status_checks.contexts, enforce_admins:.enforce_admins.enabled, required_pull_request_reviews, restrictions}' | jq .
```

If your token lacks admin perms, run the command locally and paste the JSON here for posterity.

Snapshot last updated: TBD

Notes:
- Required checks should include positive/negative offline bundle verify and the review-lock guard.
- CODEOWNERS is enforced for `bootstrapper/`, `contracts/`, and CI workflow changes.

