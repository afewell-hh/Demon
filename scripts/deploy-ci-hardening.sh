#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C.UTF-8
export GH_PAGER=

# Friendly failure dump
trap 'status=$?; if [[ $status -ne 0 && -f audit.log ]]; then echo "---- audit.log (last 80 lines) ----"; tail -n 80 audit.log; fi; exit $status' ERR

# --- ZERO-FRICTION GUARDS ---
# Required CLIs present?
for bin in gh jq git; do
  command -v "$bin" >/dev/null || { echo "❌ missing dependency: $bin"; exit 1; }
done

# Ensure we're in the right repo (belt-and-suspenders)
REPO_EXPECTED="${REPO_EXPECTED:-afewell-hh/Demon}"
REPO_ACTUAL=$(gh repo view --json nameWithOwner -q .nameWithOwner)
[[ "$REPO_ACTUAL" == "$REPO_EXPECTED" ]] || {
  echo "❌ repo mismatch: expected '$REPO_EXPECTED' but got '$REPO_ACTUAL'"
  exit 1
}

# --- SANITY CHECKS ---
gh auth status >/dev/null
echo "✓ Repo: $REPO_ACTUAL @ $(gh repo view --json defaultBranchRef -q .defaultBranchRef.name)"

# Ensure default branch really is "main" (fail fast if not)
DEFAULT_BRANCH=$(gh repo view --json defaultBranchRef -q .defaultBranchRef.name)
[[ "$DEFAULT_BRANCH" == "main" ]] || { echo "❌ default branch is '$DEFAULT_BRANCH' (expected 'main')"; exit 1; }

# Confirm the three required checks exist before merge
REQ1='Bootstrapper bundles — verify (offline, signature ok)'
REQ2='Bootstrapper bundles — negative verify (tamper ⇒ failed)'
REQ3='review-lock-guard'
PROT_JSON=$(gh api repos/:owner/:repo/branches/main/protection || echo '{}')
printf '%s\n' "$PROT_JSON" | jq -e --arg a "$REQ1" --arg b "$REQ2" --arg c "$REQ3" '
  (.required_status_checks.contexts // []) as $ctx
  | ($ctx | index($a)) and ($ctx | index($b)) and ($ctx | index($c))
' >/dev/null || { echo "❌ required checks not present in protection rule"; exit 1; }

# Strict & linear preflight (fail fast if protections drifted)
jq -e '.required_status_checks.strict == true' <<<"$PROT_JSON" >/dev/null \
  || { echo "❌ branch protection 'Require branches to be up to date' not enabled"; exit 1; }
jq -e '.required_linear_history.enabled == true' <<<"$PROT_JSON" >/dev/null \
  || { echo "❌ branch protection 'Require linear history' not enabled"; exit 1; }

# Review-lock preflight (catch drift before merges)
check_review_lock() {
  local pr="$1"
  local head sha_ok
  head=$(gh pr view "$pr" --json headRefOid -q .headRefOid)
  sha_ok=$(gh pr view "$pr" --json body -q .body | grep -F "$head" || true)
  [[ -n "$sha_ok" ]] || { echo "❌ PR #$pr Review-lock mismatch (HEAD $head not in PR body)"; exit 1; }
}

# Env-override for PR list
PRS=(${PRS_OVERRIDE:-44 42 43})
for pr in "${PRS[@]}"; do
  check_review_lock "$pr"
done

gh pr view "${PRS[@]}" --json number,isDraft,mergeable,reviewDecision \
  -q '.[]|"\(.number): draft=\(.isDraft) mergeable=\(.mergeable) review=\(.reviewDecision)"'

# --- AUTO-WAITER WITH DIAGNOSTICS ---
while :; do
  ok=1
  for pr in "${PRS[@]}"; do
    read -r isDraft mergeable review <<<"$(gh pr view "$pr" --json isDraft,mergeable,reviewDecision -q '.isDraft,.mergeable,.reviewDecision' | paste -sd' ' -)"
    if ! ([[ "$isDraft" == "false" ]] && [[ "$mergeable" == "MERGEABLE" || "$mergeable" == "true" ]] && [[ "$review" == "APPROVED" ]]); then
      echo "…waiting: PR #$pr (draft=$isDraft mergeable=$mergeable review=$review)"
      ok=0
    fi
  done
  (( ok )) && break
  sleep 15
done
echo "✓ All PRs approved! Executing merge sequence..."

# --- MERGE WITH RE-RUN SAFETY ---
merge_safe() { 
  local pr="$1" desc="$2"
  if gh pr view "$pr" --json state -q .state 2>/dev/null | grep -qi '^MERGED'; then
    echo "ℹ️ PR #$pr already merged; skipping"
  else
    echo "Merging PR #$pr: $desc"
    gh pr merge "$pr" --squash --delete-branch
  fi
}

merge_safe 44 "protection-audit enhancements + playbook"
merge_safe 42 "audit failure alerting"
merge_safe 43 "supply-chain guards"

# --- VERIFY (kick audit, wait, capture + machine-check) ---
RUN_ID=$(gh workflow run protection-audit.yml --json run -q .run.id)
echo "Started protection-audit run: $RUN_ID"
gh run watch "$RUN_ID"
gh run view "$RUN_ID" --log | tee audit.log

# Hard fail if audit has no logs
[[ -s audit.log ]] || { echo "❌ audit.log is empty; audit run produced no logs"; exit 1; }

# Unicode-exact greps
grep -F 'Bootstrapper bundles — verify (offline, signature ok)' audit.log \
 && grep -F 'Bootstrapper bundles — negative verify (tamper ⇒ failed)' audit.log \
 && grep -F 'review-lock-guard' audit.log \
 && grep -E 'required_status_checks.+strict.+true' audit.log \
 && grep -E 'required_linear_history.+enabled.+true' audit.log \
 && echo "✓ Protection audit: checks + strict + linear history verified"

# Structured double-check (post-audit)
echo "Double-checking via API..."
gh api repos/:owner/:repo/branches/main/protection \
  -q '.required_status_checks as $r | [$r.contexts[], .required_pull_request_reviews.require_code_owner_reviews]' \
  | jq -s 'add | sort'

# --- IDEMPOTENT SNAPSHOT (governance) ---
git config --global user.email "${GIT_USER_EMAIL:-ops@example.com}"
git config --global user.name  "${GIT_USER_NAME:-demon-ci-ops}"

mkdir -p .github/snapshots
SNAP=".github/snapshots/branch-protection-$(date -u +%F).json"
gh api repos/:owner/:repo/branches/main/protection > "$SNAP" || true

if ! git diff --quiet -- "$SNAP"; then
  git add "$SNAP"
  git commit -m "governance: snapshot branch protection ($(date -u +%F))"
  git push
else
  echo "ℹ️ snapshot unchanged; skipping commit"
fi

echo "✅ CI HARDENING DEPLOYED SUCCESSFULLY"