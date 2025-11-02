#!/usr/bin/env bash
set -euo pipefail

# Poll PR #284 until it is merged, then post a "freeze in effect" comment to Issue #285

PR_NUMBER=284
ISSUE_NUMBER=285
POLL_SECS=${POLL_SECS:-60}

echo "[freeze-watch] Watching PR #$PR_NUMBER for merge… (poll ${POLL_SECS}s)"

while true; do
  JSON=$(gh pr view "$PR_NUMBER" --json mergedAt,mergeCommit,url,state,mergeable,reviewDecision 2>/dev/null || echo '{}')
  MERGED_AT=$(jq -r '.mergedAt // empty' <<<"$JSON" || true)
  MERGE_SHA=$(jq -r '.mergeCommit.oid // empty' <<<"$JSON" || true)
  STATE=$(jq -r '.state // empty' <<<"$JSON" || true)
  MERGEABLE=$(jq -r '.mergeable // empty' <<<"$JSON" || true)
  REVIEW=$(jq -r '.reviewDecision // empty' <<<"$JSON" || true)

  if [[ -n "$MERGED_AT" && -n "$MERGE_SHA" ]]; then
    echo "[freeze-watch] PR merged at $MERGED_AT (sha $MERGE_SHA)"
    REPO_URL=$(gh repo view --json url -q .url 2>/dev/null || echo "https://github.com/afewell-hh/Demon")
    SSE_GRAPH_ANCHOR="$REPO_URL/blob/main/docs/api/graph.md#stream-commits-sse"
    SSE_RITUAL_ANCHOR="$REPO_URL/blob/main/docs/app-packs/ritual-http-api.md"
    ISSUE_286_URL="$REPO_URL/issues/286"
    NOTE=$(cat <<EOF
Freeze in effect — PR #$PR_NUMBER merged.

- Merged At: $MERGED_AT
- Merge SHA:

\`\`\`
$MERGE_SHA
\`\`\`

References:
- SSE client docs: $SSE_GRAPH_ANCHOR
- Related: #286

All non-essential changes are frozen until the unfreeze notice is posted. Docs-only PRs may proceed; ensure required guards remain green and reply to all review threads.
EOF
)
    gh issue comment "$ISSUE_NUMBER" --body "$NOTE"
    echo "[freeze-watch] Posted freeze note to issue #$ISSUE_NUMBER. Exiting."
    exit 0
  fi

  if [[ "$STATE" == "CLOSED" ]]; then
    echo "[freeze-watch] PR closed without merge; exiting."
    exit 0
  fi

  # Optional: attempt merge if clearly ready but stalled
  if [[ "${DO_MERGE_IF_READY:-0}" == "1" && "$STATE" == "OPEN" && "$MERGEABLE" == "MERGEABLE" ]]; then
    echo "[freeze-watch] Attempting to merge PR #$PR_NUMBER (mergeable=$MERGEABLE review=$REVIEW)…"
    gh pr merge "$PR_NUMBER" --merge --delete-branch=false --body "Merging per freeze request; will post freeze note with SHA on #$ISSUE_NUMBER." || true
  fi

  sleep "$POLL_SECS"
done
