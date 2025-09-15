#!/bin/bash
set -euo pipefail

echo "Adding labels to issues #56-#63 as a workaround for project field permissions"

# Backend issues: #56-#59
for issue in 56 57 58 59; do
  echo "Labeling issue #$issue (backend)..."
  gh issue edit $issue --add-label "area:backend,p0,MVP-Alpha" 2>/dev/null || echo "  Some labels may already exist"
done

# Frontend issues: #60-#63
for issue in 60 61 62 63; do
  echo "Labeling issue #$issue (frontend)..."
  gh issue edit $issue --add-label "area:frontend,p0,MVP-Alpha" 2>/dev/null || echo "  Some labels may already exist"
done

echo "Done! Issues have been labeled. You'll need to manually update project fields or use a token with project permissions."
