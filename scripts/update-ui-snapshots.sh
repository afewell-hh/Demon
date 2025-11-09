#!/usr/bin/env bash
set -euo pipefail

# UI Snapshot Update Script
# Regenerates Playwright visual snapshots for Operate UI
#
# Usage:
#   ./scripts/update-ui-snapshots.sh             # Update all snapshots
#   ./scripts/update-ui-snapshots.sh --help      # Show help
#
# Requirements:
#   - NATS running on port 4222 (or NATS_PORT env var)
#   - operate-ui server running on port 3000 (or BASE_URL env var)
#   - npm and Playwright installed in operate-ui/playwright/

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PLAYWRIGHT_DIR="$REPO_ROOT/operate-ui/playwright"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Help text
show_help() {
  cat << EOF
UI Snapshot Update Script

Updates Playwright visual snapshots for Operate UI regression testing.

USAGE:
  $0 [OPTIONS]

OPTIONS:
  -h, --help     Show this help message

PREREQUISITES:
  1. NATS JetStream running on port 4222
     Start with: make up

  2. Operate UI server running on port 3000
     Start with: OPERATE_UI_FLAGS=contracts-browser,canvas-ui cargo run -p operate-ui

  3. Seed preview data (optional but recommended)
     Run: ./examples/seed/seed_preview.sh

ENVIRONMENT VARIABLES:
  BASE_URL       Base URL for operate-ui (default: http://localhost:3000)
  NATS_PORT      NATS port if not 4222
  NATS_URL       Override NATS URL entirely

EXAMPLES:
  # Basic usage (assumes services running):
  ./scripts/update-ui-snapshots.sh

  # With custom base URL:
  BASE_URL=http://localhost:8080 ./scripts/update-ui-snapshots.sh

NOTES:
  - Snapshots are stored in operate-ui/tests/__artifacts__/snapshots/
  - After updating, review diffs carefully before committing
  - Commit updated snapshots with your UI changes
  - See docs/process/ui_snapshot_workflow.md for workflow details

EOF
}

# Parse arguments
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  show_help
  exit 0
fi

# Check prerequisites
echo -e "${GREEN}Checking prerequisites...${NC}"

# Check if playwright directory exists
if [[ ! -d "$PLAYWRIGHT_DIR" ]]; then
  echo -e "${RED}Error: Playwright directory not found at $PLAYWRIGHT_DIR${NC}"
  exit 1
fi

# Check if npm dependencies are installed
if [[ ! -d "$PLAYWRIGHT_DIR/node_modules" ]]; then
  echo -e "${YELLOW}Installing npm dependencies...${NC}"
  cd "$PLAYWRIGHT_DIR"
  npm install
  npx playwright install --with-deps
fi

# Check if operate-ui is running
BASE_URL="${BASE_URL:-http://localhost:3000}"
if ! curl -sf "$BASE_URL/api/runs" >/dev/null 2>&1; then
  echo -e "${RED}Error: Operate UI does not appear to be running at $BASE_URL${NC}"
  echo -e "${YELLOW}Start it with: OPERATE_UI_FLAGS=contracts-browser,canvas-ui cargo run -p operate-ui${NC}"
  exit 1
fi

echo -e "${GREEN}✓ Operate UI is running at $BASE_URL${NC}"

# Check if NATS is accessible (optional but recommended)
NATS_URL="${NATS_URL:-nats://127.0.0.1:${NATS_PORT:-4222}}"
# We can't easily check NATS without a client, so just note it
echo -e "${YELLOW}Note: Ensure NATS is running at $NATS_URL${NC}"
echo -e "${YELLOW}Note: For best results, seed data with ./examples/seed/seed_preview.sh${NC}"

# Run Playwright with update snapshots flag
echo ""
echo -e "${GREEN}Updating UI snapshots...${NC}"
cd "$PLAYWRIGHT_DIR"

# Set environment variable to force update all snapshots
export UPDATE_SNAPSHOTS=true
export BASE_URL="$BASE_URL"

# Run only snapshot tests (filter by test name pattern)
npx playwright test --grep "visual snapshot" --update-snapshots

RESULT=$?

if [[ $RESULT -eq 0 ]]; then
  echo ""
  echo -e "${GREEN}✓ Snapshots updated successfully!${NC}"
  echo ""
  echo "Updated snapshots are in: operate-ui/tests/__artifacts__/snapshots/"
  echo ""
  echo "Next steps:"
  echo "  1. Review the changes: git diff operate-ui/tests/__artifacts__/snapshots/"
  echo "  2. Commit the updated snapshots with your UI changes"
  echo "  3. Reference this script in your PR description"
  echo ""
else
  echo ""
  echo -e "${RED}✗ Snapshot update failed with exit code $RESULT${NC}"
  echo ""
  echo "Troubleshooting:"
  echo "  - Check that Operate UI is running with correct feature flags"
  echo "  - Verify NATS is accessible"
  echo "  - Check Playwright logs above for specific errors"
  echo "  - See docs/process/ui_snapshot_workflow.md"
  echo ""
  exit $RESULT
fi
