#!/usr/bin/env bash
set -euo pipefail

# Audit unresolved PR review threads (and no-reply threads)
# Usage:
#   bash audit-pr-threads.sh            # scans last 30 PRs (default)
#   COUNT=100 bash audit-pr-threads.sh  # scan last 100 PRs
#
# Requirements: gh (authenticated), jq

OWNER=${OWNER:-afewell-hh}
REPO=${REPO:-Demon}
COUNT=${COUNT:-30}

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

JSON=$(gh api graphql -f owner=$OWNER -f name=$REPO -F n=$COUNT -f query="$Q")

echo "PR,State,UnresolvedThreads,UnresolvedNoReplyThreads,URL"
jq -r '
  .data.repository.pullRequests.nodes[]
  | . as $pr
  | ($pr.reviewThreads.nodes // []) as $threads
  | ($threads
      | map(select(.isResolved==false))
    ) as $unresolved
  | ($unresolved
      | map(
          {noReply:
            ( ( .comments.totalCount // 0 ) <= 1
              or ( .comments.nodes // [] | map(.author.login) | length==1 )
            ),
           firstUrl: ( .comments.nodes[0].url // $pr.url )
          }
        )
    ) as $marks
  | [$pr.number,
     $pr.state,
     ($unresolved | length),
     ($marks | map(select(.noReply==true)) | length),
     $pr.url
    ]
  | @csv
' <<<"$JSON" | sed '1!b;s/\"//g'
