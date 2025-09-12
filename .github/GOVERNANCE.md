# Governance & Protection Status

## Branch Protection Configuration
**Last verified:** 2025-09-12  
**Status:** ✅ ACTIVE

### Required Checks (main branch)
- `Bootstrapper bundles — verify (offline, signature ok)`
- `Bootstrapper bundles — negative verify (tamper ⇒ failed)`
- `review-lock-guard`

### Protection Settings
- ✅ Require pull request before merging
- ✅ Require review from Code Owners  
- ✅ Dismiss stale approvals on new commits
- ✅ Require status checks to pass
- ✅ Require linear history
- ✅ Require branches to be up to date

## Secrets & PAT Management
**PROTECTION_TOKEN**
- **Purpose:** Protection audit workflow (Administration: Read-only)
- **Created:** 2025-09-12 (using GitHub CLI token)
- **Next rotation:** 2026-01-12 (quarterly)
- **Owner:** afewell-hh

## Ops Quick-Refs

### Re-run protection audit
```bash
# Trigger audit (if workflow_dispatch enabled)
gh workflow run protection-audit.yml
gh run view "$(gh run list --workflow protection-audit.yml --limit 1 --json databaseId -q '.[0].databaseId')" --log
```

### Manual protection check
```bash
# Quick API sanity check
gh api repos/:owner/:repo/branches/main/protection \
  -q '.required_status_checks.contexts, .required_pull_request_reviews.require_code_owner_reviews' | jq .
```

### Snapshot branch protection
```bash
# Create dated snapshot
SNAP=".github/snapshots/branch-protection-$(date -u +%F).json"
gh api repos/:owner/:repo/branches/main/protection > "$SNAP"
git add "$SNAP" && git commit -m "governance: snapshot branch protection ($(date -u +%F))" && git push
```

### Drift drill (test protection)
```bash
# Open test PR with wrong review-lock SHA
echo "Test content" > test.txt
git checkout -b test/review-lock-$(date +%s)
git add test.txt && git commit -m "test: review-lock guard"
gh pr create --title "Test: review-lock guard" --body "Review-lock: 0000000000000000000000000000000000000000"
# Should fail review-lock-guard check
```

### Verify bootstrapper (positive/negative)
```bash
# Positive verify (should pass)
cargo run -p bootstrapper-demonctl -- --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
  | jq -e 'select(.phase=="verify" and .signature=="ok")'

# Negative verify (tamper test)
cp examples/bundles/local-dev.yaml{,.bak}
sed -i 's/duplicateWindowSeconds: 120/duplicateWindowSeconds: 999/' examples/bundles/local-dev.yaml
cargo run -p bootstrapper-demonctl -- --verify-only --bundle lib://local/preview-local-dev@0.0.1 \
  | jq -e 'select(.phase=="verify" and .signature=="failed")'
mv examples/bundles/local-dev.yaml{.bak,}
```

## Maintenance Schedule
- **Monthly:** Run protection audit + commit snapshot
- **Quarterly:** Rotate PROTECTION_TOKEN
- **On CI changes:** Re-verify job names match protection requirements

## Protection History
- 2025-09-12: Initial protection enabled (PR #38, #40)
- 2025-09-12: First governance snapshot committed

---
*Job names are frozen in CI with "DO NOT RENAME" comments to maintain protection stability.*
