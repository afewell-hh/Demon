#!/usr/bin/env bash
set -euo pipefail

# Validation script for GHCR digest workflow (issue #231)
# Run after docker-build.yml completes successfully on main

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-/tmp/ghcr-validation-$(date +%s)}"

mkdir -p "$OUTPUT_DIR"

echo "📋 GHCR Digest Validation Workflow"
echo "=================================="
echo "Output directory: $OUTPUT_DIR"
echo ""

# Ensure GH_TOKEN is set
if [[ -z "${GH_TOKEN:-}" ]]; then
    echo "⚠️  GH_TOKEN not set, attempting to use gh auth token"
    export GH_TOKEN=$(gh auth token)
fi

echo "✅ GH_TOKEN configured"
echo ""

# Step 1: Fetch digests from latest successful docker-build run on main
echo "Step 1: Fetching digests from docker-build.yml on main..."
echo "-----------------------------------------------------------"

cargo run -p demonctl -- docker digests fetch \
  --workflow docker-build.yml \
  --branch main \
  --format env \
  --output "$OUTPUT_DIR/docker-image-digests.json" \
  | tee "$OUTPUT_DIR/ghcr-digests.env"

echo ""
echo "✅ Digests saved to:"
echo "   - JSON: $OUTPUT_DIR/docker-image-digests.json"
echo "   - ENV:  $OUTPUT_DIR/ghcr-digests.env"
echo ""

# Step 2: Source the environment variables
echo "Step 2: Sourcing environment variables..."
echo "-----------------------------------------------------------"
source "$OUTPUT_DIR/ghcr-digests.env"

echo "OPERATE_UI_IMAGE_TAG=${OPERATE_UI_IMAGE_TAG:-<not set>}"
echo "RUNTIME_IMAGE_TAG=${RUNTIME_IMAGE_TAG:-<not set>}"
echo "ENGINE_IMAGE_TAG=${ENGINE_IMAGE_TAG:-<not set>}"
echo ""

# Step 3: Dry-run bootstrap with fetched digests
echo "Step 3: Running dry-run bootstrap with fetched digests..."
echo "-----------------------------------------------------------"

cargo run -p demonctl -- k8s-bootstrap bootstrap \
  --config scripts/tests/fixtures/config.e2e.yaml \
  --dry-run \
  --verbose \
  | tee "$OUTPUT_DIR/bootstrap-dryrun-output.txt"

echo ""
echo "✅ Dry-run output saved to: $OUTPUT_DIR/bootstrap-dryrun-output.txt"
echo ""

# Step 4: Test --use-latest-digests flag
echo "Step 4: Testing --use-latest-digests flag..."
echo "-----------------------------------------------------------"

# Unset env vars to prove --use-latest-digests fetches them
unset OPERATE_UI_IMAGE_TAG RUNTIME_IMAGE_TAG ENGINE_IMAGE_TAG

cargo run -p demonctl -- k8s-bootstrap bootstrap \
  --config scripts/tests/fixtures/config.e2e.yaml \
  --dry-run \
  --use-latest-digests \
  --workflow docker-build.yml \
  --branch main \
  | tee "$OUTPUT_DIR/bootstrap-use-latest-digests-output.txt"

echo ""
echo "✅ --use-latest-digests output saved to: $OUTPUT_DIR/bootstrap-use-latest-digests-output.txt"
echo ""

# Summary
echo "🎉 Validation Complete"
echo "======================"
echo ""
echo "Artifacts:"
echo "  - Digest JSON:              $OUTPUT_DIR/docker-image-digests.json"
echo "  - Digest ENV:               $OUTPUT_DIR/ghcr-digests.env"
echo "  - Bootstrap dry-run:        $OUTPUT_DIR/bootstrap-dryrun-output.txt"
echo "  - --use-latest-digests:     $OUTPUT_DIR/bootstrap-use-latest-digests-output.txt"
echo ""
echo "Next steps:"
echo "  1. Review outputs in $OUTPUT_DIR"
echo "  2. Update issue #231 with validation results"
echo "  3. Update issue #228 comment with references to new docs"
echo ""
