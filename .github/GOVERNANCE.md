# Governance & Protection Status

## Branch Protection Configuration
**Last verified:** 2025-09-16  
**Status:** ✅ ACTIVE

### Required Checks (main branch)
- `Bootstrapper bundles — verify (offline, signature ok)`
- `Bootstrapper bundles — negative verify (tamper ⇒ failed)`
- `review-lock-guard`

### Protection Settings
- ✅ Require pull request before merging
- ❌ Require review from Code Owners  
- ✅ Dismiss stale approvals on new commits
- ✅ Require status checks to pass (strict = false)
- ✅ Require linear history
- ❌ Require branches to be up to date (strict = false)

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

## Protected Tags

### Recommended Tag Protection Rules
Protect release and preview tags from unauthorized changes:
- **Pattern**: `v*` (version tags like v1.0.0)
- **Pattern**: `preview-*` (preview releases)

### Settings
- ❌ Forbid force-push to protected tags
- ❌ Forbid direct creation (require PR → merge → tag)
- ✅ Allow deletion only by maintainers

### How to Enable Tag Protection
1. Go to Settings → Tags → Add rule
2. Enter pattern (e.g., `v*`)
3. Select "Restrict who can create matching tags"
4. Add maintainer team/users who can create tags
5. Save rule

*Alternative: Use Rulesets for more granular control (Settings → Rules → Rulesets)*

## Emergency Playbook

### CI is Red — What Now?

#### If protection-audit fails
1. Open the failed run → read the assertion error
2. If a required check name drifted: align CI job name (never rename in Branch Protection)
3. If PROTECTION_TOKEN returns 403: rotate PAT or re-grant Administration: Read-only
4. Re-run audit from Actions tab; ensure it passes

#### If cargo-audit breaks (new advisory)
1. Open a chore PR to bump affected dependencies
2. If fix unavailable: temporarily allowlist with expiry date
   ```toml
   # deny.toml
   [advisories]
   ignore = ["RUSTSEC-2025-XXXX"] # TODO: remove after fix released
   ```
3. Document exception in PR with follow-up ticket link

#### If a release needs to land while non-critical guard is red
1. Keep required checks intact — do not disable protection
2. Land targeted fix PR addressing only the failure
3. Document exception in PR body with evidence and follow-up ticket

#### Rollback Procedure
```bash
# For squash merges (revert cleanly)
git revert <merge_sha>
git push origin main

# NEVER disable protection or rename required jobs to "make it green"
```

## Protection History
- 2025-09-12: Initial protection enabled (PR #38, #40)
- 2025-09-12: First governance snapshot committed
- 2025-09-12: Added tag protection docs and emergency playbook

---
*Job names are frozen in CI with "DO NOT RENAME" comments to maintain protection stability.*
