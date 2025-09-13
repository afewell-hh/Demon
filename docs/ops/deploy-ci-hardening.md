# Deploy CI Hardening - Quick Reference Card

## Quick-start
```bash
# 1. Authenticate as repo admin
gh auth status

# 2. Confirm CODEOWNERS approvals on target PRs

# 3. Run from repo root
bash scripts/deploy-ci-hardening.sh
```

## Tunables
```bash
REPO_EXPECTED=afewell-hh/Demon        # Override for forks
PRS_OVERRIDE="44 42 43"               # If PR numbers change
GIT_USER_EMAIL=ops@example.com        # Snapshot commit author
GIT_USER_NAME=demon-ci-ops            # Snapshot commit name
```

## Script Guarantees
✅ Repo & branch verification  
✅ Required checks & protection settings preflight  
✅ Review-lock validation before any action  
✅ Approval wait → ordered merge → audit → verify  
✅ Unicode-exact machine checks  
✅ Idempotent snapshot (only commits changes)  

## Success Indicators
```
✓ Protection audit: checks + strict + linear history verified
✅ CI HARDENING DEPLOYED SUCCESSFULLY
```

## Fast Triage
| Issue | Fix |
|-------|-----|
| Missing check names | Restore frozen CI job labels, re-run |
| 403 on protection API | Check PROTECTION_TOKEN scope |
| Review-lock mismatch | Update PR body with HEAD SHA |
| Empty audit.log | Re-run workflow manually, check YAML |

## Optional Enhancements

**Dry-run mode:**
```bash
DRY_RUN=1 bash scripts/deploy-ci-hardening.sh
```

**Makefile target:**
```bash
make deploy-ci-hardening
```

## What It Does

1. **Verifies environment**: Checks CLI dependencies, repo, branch, auth
2. **Validates protections**: Ensures required checks, strict mode, linear history
3. **Checks review-locks**: Confirms HEAD SHA in each PR body
4. **Waits for approvals**: Monitors until all PRs are CODEOWNERS approved
5. **Merges in order**: 44 → 42 → 43 (protection-audit → alerts → supply-chain)
6. **Runs verification**: Triggers protection-audit workflow and validates output
7. **Creates snapshot**: Saves branch protection state to `.github/snapshots/`

## Architecture

This tool deploys three complementary CI security enhancements:

- **PR #44**: Enhanced protection-audit with strict/linear history validation + emergency playbook
- **PR #42**: Automated alerting (GitHub Issues + optional Slack) on protection failures  
- **PR #43**: Supply-chain security guards (cargo-audit + cargo-deny) with curated license policies

The deployment is atomic and idempotent - safe to re-run if interrupted.