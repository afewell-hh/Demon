#!/bin/bash

# Smoke verification script for contract bundle releases
# This script downloads and validates a contract bundle release

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Default values
RELEASE_TAG="${1:-contracts-latest}"
WORK_DIR="${2:-/tmp/release-verify-$(date +%s)}"
DEMONCTL_PATH="${3:-$PROJECT_ROOT/target/debug/demonctl}"

echo "🧪 Smoke testing contract bundle release: $RELEASE_TAG"
echo "📁 Working directory: $WORK_DIR"
echo "🛠️  Using demonctl: $DEMONCTL_PATH"

# Create and enter work directory
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

# Download released assets
echo "📥 Downloading release assets..."
if ! gh release download "$RELEASE_TAG" \
    -p "bundle.json" \
    -p "manifest.json" \
    -p "bundle.sha256"; then
    echo "❌ Failed to download release assets for $RELEASE_TAG"
    exit 1
fi

echo "✓ Downloaded release assets"

# Verify SHA-256 checksum
echo "🔐 Verifying bundle integrity..."
if ! shasum -a 256 -c bundle.sha256; then
    echo "❌ Bundle SHA-256 verification failed!"
    exit 1
fi
echo "✓ Bundle integrity verified"

# Validate bundle structure using demonctl
echo "✅ Validating bundle structure..."
if ! "$DEMONCTL_PATH" contracts validate-envelope --stdin < bundle.json; then
    echo "❌ Bundle validation failed!"
    exit 1
fi
echo "✓ Bundle validation successful"

# Additional smoke tests
echo "🔍 Running additional smoke tests..."

# Check manifest contains expected fields
echo "  Checking manifest fields..."
if ! jq -e '.version' manifest.json >/dev/null; then
    echo "❌ Missing version in manifest"
    exit 1
fi

if ! jq -e '.bundle_sha256' manifest.json >/dev/null; then
    echo "❌ Missing bundle_sha256 in manifest"
    exit 1
fi

if ! jq -e '.git.sha' manifest.json >/dev/null; then
    echo "❌ Missing git.sha in manifest"
    exit 1
fi

echo "  ✓ Manifest contains required fields"

# Check bundle contains expected schemas
echo "  Checking bundle schema content..."
if ! jq -e '.schemas."result-envelope.json"' bundle.json >/dev/null; then
    echo "❌ Missing result-envelope schema in bundle"
    exit 1
fi

if ! jq -e '.schemas."bootstrap.bundle.v0.json"' bundle.json >/dev/null; then
    echo "❌ Missing bootstrap.bundle schema in bundle"
    exit 1
fi

echo "  ✓ Bundle contains expected schemas"

# Verify manifest SHA matches actual bundle SHA
echo "  Cross-checking manifest SHA..."
ACTUAL_SHA=$(shasum -a 256 bundle.json | cut -d' ' -f1)
MANIFEST_SHA=$(jq -r '.bundle_sha256' manifest.json)

if [[ "$ACTUAL_SHA" != "$MANIFEST_SHA" ]]; then
    echo "❌ SHA-256 mismatch between manifest and actual bundle!"
    echo "  Manifest SHA: $MANIFEST_SHA"
    echo "  Actual SHA:   $ACTUAL_SHA"
    exit 1
fi

echo "  ✓ Manifest SHA matches bundle SHA"

# Extract and display release metadata
echo ""
echo "📊 Release Summary:"
echo "  Tag: $RELEASE_TAG"
echo "  Version: $(jq -r '.version' manifest.json)"
echo "  Timestamp: $(jq -r '.timestamp' manifest.json)"
echo "  Git SHA: $(jq -r '.git.sha' manifest.json)"
echo "  Bundle SHA-256: $(jq -r '.bundle_sha256' manifest.json)"
echo "  Bundle Size: $(du -h bundle.json | cut -f1)"
echo "  Manifest Size: $(du -h manifest.json | cut -f1)"

echo ""
echo "✅ All smoke tests passed!"
echo "🎉 Release $RELEASE_TAG is healthy and ready for consumption"

# Cleanup unless we're in CI (preserve for debugging)
if [[ "${CI:-}" != "true" ]]; then
    echo ""
    echo "🧹 Cleaning up work directory: $WORK_DIR"
    cd "$PROJECT_ROOT"
    rm -rf "$WORK_DIR"
fi

exit 0# This comment forces a new review-lock check
