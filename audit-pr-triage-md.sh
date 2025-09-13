#!/usr/bin/env bash
set -euo pipefail

# deps
for bin in gh jq; do
  command -v "$bin" >/dev/null || { echo "missing dependency: $bin"; exit 1; }
done

OWNER=${OWNER:-afewell-hh}
REPO=${REPO:-Demon}
COUNT=${COUNT:-30}   # scan last N PRs
OUTDIR=${OUTDIR:-.}
STAMP=$(date -u +%F)
OUT="${OUTDIR}/pr-review-triage-${STAMP}.md"

read -r -d '' Q <<'GRAPHQL' || true
query($owner:String!, $name:String!, $n:Int!) {
  repository(owner:$owner, name:$name){
    pullRequests(last:$n, states:[OPEN, MERGED, CLOSED], orderBy:{field:UPDATED_AT, direction:DESC}) {
      nodes {
        number title state mergedAt url author { login }
        reviewThreads(first:100) {
          nodes {
            isResolved
            isOutdated
            comments(first:50) {
              totalCount
              nodes {
                author { login }
                url
                bodyText
                publishedAt
              }
            }
          }
        }
      }
    }
  }
}
GRAPHQL

JSON=$(gh api graphql -f owner="$OWNER" -f name="$REPO" -F n="$COUNT" -f query="$Q")

# Build Markdown
{
  echo "# Review Triage Report — last ${COUNT} PRs"
  echo
  echo "_Generated: ${STAMP} UTC — repo: ${OWNER}/${REPO}_"
  echo
  jq -r '
    def trunc(s; n):
      if s == null then "" else
        if (s|length) > n then (s[0:n] + "…") else s end
      end;

    .data.repository.pullRequests.nodes[]
    | . as $pr
    | ($pr.reviewThreads.nodes // []) as $threads
    | ($threads | map(select(.isResolved==false))) as $unres
    | ($unres
        | map({
            first: (.comments.nodes[0]?),
            last:  (.comments.nodes[-1]?),
            total: (.comments.totalCount // 0),
            authorPresent: ((.comments.nodes // []) | any(.author.login == $pr.author.login)),
            firstUrl: (.comments.nodes[0]?.url),
            firstText: trunc(.comments.nodes[0]?.bodyText; 200),
            firstAuthor: (.comments.nodes[0]?.author.login),
            lastAuthor:  (.comments.nodes[-1]?.author.login),
            lastUrl: (.comments.nodes[-1]?.url)
          })
      ) as $details
    | ($details | map(select(.authorPresent|not)) | length) as $noReplyCount
    |
    "## PR #" + ($pr.number|tostring) + ": " + $pr.title,
    "- URL: " + $pr.url,
    "- Author: @" + $pr.author.login + " | State: " + $pr.state + (if $pr.mergedAt then " | MergedAt: " + $pr.mergedAt else "" end),
    "- Unresolved threads: " + (($unres|length)|tostring) + " | No-reply: " + ($noReplyCount|tostring),
    ( if ($unres|length) == 0
      then "\n"
      else
        ( $details[]
          | "* " +
            (if .authorPresent then "" else "**NO-REPLY** · " end) +
            "[thread](" + (.firstUrl // $pr.url) + ") · " +
            (if .total then (.total|tostring) + " comment(s) · " else "" end) +
            "first by @" + ((.firstAuthor // "n/a")) +
            (if .lastAuthor and .lastUrl and (.lastUrl != .firstUrl)
               then " · last by @" + .lastAuthor
               else ""
             end) +
            "\n  > " + ((.firstText // "")|gsub("\n"; " ") )
        )
      end
    ),
    ""  # spacer
  ' <<<"$JSON"
  cat <<'TEMPLATES'

---

## Quick reply snippets

- **Fixed:** _“Addressed in `<short-sha>`; please re-review.”_
- **Clarified:** _“Added explanation in docs/comments at `<link>`.”_
- **Won’t fix (reason):** _“Won’t fix because `<reason>`; tracking in #<issue>.”_

> Close each thread after replying (or push a fix) to keep the audit trail clean.

TEMPLATES
} > "$OUT"

echo "wrote $OUT"

