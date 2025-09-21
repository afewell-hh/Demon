#!/bin/bash
# validate-release.sh - Validate contract bundle release
# Usage: ./scripts/validate-release.sh [TAG]
# Example: ./scripts/validate-release.sh contracts-latest

set -euo pipefail

# Configuration
TAG=${1:-contracts-latest}
WORKDIR=$(mktemp -d)
DEMONCTL_BIN=${DEMONCTL_BIN:-"cargo run -p demonctl --"}
VERBOSE=${VERBOSE:-false}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date '+%H:%M:%S')]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[$(date '+%H:%M:%S')] WARN:${NC} $1"
}

error() {
    echo -e "${RED}[$(date '+%H:%M:%S')] ERROR:${NC} $1"
}

verbose() {
    if [ "$VERBOSE" = "true" ]; then
        echo -e "${YELLOW}[DEBUG]${NC} $1"
    fi
}

cleanup() {
    if [ -n "${WORKDIR:-}" ] && [ -d "$WORKDIR" ]; then
        verbose "Cleaning up temporary directory: $WORKDIR"
        rm -rf "$WORKDIR"
    fi
}

# Trap to ensure cleanup happens
trap cleanup EXIT

main() {
    log "🔍 Validating contract bundle release: $TAG"
    cd "$WORKDIR"

    # Step 1: Download release assets
    log "📥 Downloading release assets..."
    if ! gh release download "$TAG" -p "*.json" -p "*.sha256" 2>/dev/null; then
        error "Failed to download release $TAG"
        error "Check that the release exists and GitHub CLI is authenticated"
        exit 1
    fi

    # Step 2: Verify required files exist
    log "📋 Verifying required files..."
    local required_files=("bundle.json" "manifest.json" "bundle.sha256")
    for file in "${required_files[@]}"; do
        if [ -f "$file" ]; then
            verbose "✓ $file exists ($(du -h "$file" | cut -f1))"
        else
            error "✗ Required file missing: $file"
            exit 1
        fi
    done
    log "✅ All required files present"

    # Step 3: Verify SHA-256 integrity
    log "🔐 Verifying SHA-256 integrity..."
    if shasum -a 256 -c bundle.sha256 >/dev/null 2>&1; then
        log "✅ SHA-256 verification passed"
    else
        error "❌ SHA-256 verification failed!"
        echo "Expected checksums:"
        cat bundle.sha256
        echo "Actual checksum:"
        shasum -a 256 bundle.json
        exit 1
    fi

    # Step 4: Validate bundle structure with demonctl
    log "🏗️  Validating bundle structure..."
    if $DEMONCTL_BIN contracts validate bundle.json >/dev/null 2>&1; then
        log "✅ Bundle structure validation passed"
    else
        error "❌ Bundle structure validation failed!"
        warn "Running validation again with verbose output:"
        $DEMONCTL_BIN contracts validate bundle.json || exit 1
    fi

    # Step 5: Validate manifest metadata
    log "📄 Validating manifest metadata..."
    local required_fields=("version" "timestamp" "git.sha" "bundle_sha256")
    for field in "${required_fields[@]}"; do
        if jq -e ".$field" manifest.json >/dev/null 2>&1; then
            local value=$(jq -r ".$field" manifest.json)
            verbose "✓ $field: $value"
        else
            error "✗ Required manifest field missing: $field"
            exit 1
        fi
    done
    log "✅ Manifest metadata complete"

    # Step 6: Cross-validate manifest SHA with actual bundle
    log "🔗 Cross-validating manifest SHA..."
    local bundle_sha=$(shasum -a 256 bundle.json | cut -d' ' -f1)
    local manifest_sha=$(jq -r '.bundle_sha256' manifest.json)

    if [ "$bundle_sha" = "$manifest_sha" ]; then
        log "✅ Manifest SHA matches bundle SHA"
        verbose "SHA-256: $bundle_sha"
    else
        error "❌ Manifest SHA mismatch!"
        error "Bundle SHA-256:   $bundle_sha"
        error "Manifest SHA-256: $manifest_sha"
        exit 1
    fi

    # Step 7: Validate timestamp freshness (warning only)
    log "⏰ Checking release freshness..."
    local timestamp=$(jq -r '.timestamp' manifest.json)
    local release_time=$(date -d "$timestamp" +%s 2>/dev/null || echo "0")
    local current_time=$(date +%s)
    local age_hours=$(( (current_time - release_time) / 3600 ))

    if [ $age_hours -lt 48 ]; then
        log "✅ Release is fresh (${age_hours}h old)"
    elif [ $age_hours -lt 168 ]; then  # 1 week
        warn "⚠️  Release is aging (${age_hours}h old, consider updating)"
    else
        warn "⚠️  Release is quite old (${age_hours}h old, update recommended)"
    fi

    # Step 8: Validate Git SHA format
    log "🔖 Validating Git metadata..."
    local git_sha=$(jq -r '.git.sha' manifest.json)
    if [[ $git_sha =~ ^[a-f0-9]{40}$ ]]; then
        log "✅ Git SHA format valid: ${git_sha:0:8}..."
    else
        error "❌ Invalid Git SHA format: $git_sha"
        exit 1
    fi

    # Step 9: Summary
    log "📊 Release Summary:"
    echo "  Tag:       $TAG"
    echo "  Version:   $(jq -r '.version' manifest.json)"
    echo "  Timestamp: $(jq -r '.timestamp' manifest.json)"
    echo "  Git SHA:   $(jq -r '.git.sha' manifest.json)"
    echo "  Bundle SHA: $bundle_sha"
    echo "  Age:       ${age_hours}h"
    echo "  Size:      $(du -h bundle.json | cut -f1)"

    log "🎉 Release validation completed successfully!"
}

# Help function
show_help() {
    cat << EOF
Usage: $0 [OPTIONS] [TAG]

Validate a contract bundle release for integrity and correctness.

ARGUMENTS:
    TAG                 Release tag to validate (default: contracts-latest)

OPTIONS:
    -h, --help         Show this help message
    -v, --verbose      Enable verbose output

ENVIRONMENT VARIABLES:
    DEMONCTL_BIN       Path to demonctl binary (default: "cargo run -p demonctl --")
    VERBOSE            Enable verbose output (true/false)

EXAMPLES:
    $0                                    # Validate latest release
    $0 contracts-20250921-0658fb8b        # Validate specific release
    $0 --verbose contracts-latest         # Validate with verbose output
    DEMONCTL_BIN=./target/release/demonctl $0  # Use compiled binary

EXIT CODES:
    0    Validation successful
    1    Validation failed or error occurred
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -*)
            error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
        *)
            TAG="$1"
            shift
            ;;
    esac
done

# Verify dependencies
for cmd in gh jq shasum; do
    if ! command -v $cmd >/dev/null 2>&1; then
        error "Required command not found: $cmd"
        exit 1
    fi
done

# Check if demonctl is available
if ! $DEMONCTL_BIN --version >/dev/null 2>&1; then
    error "demonctl not available at: $DEMONCTL_BIN"
    error "Build the workspace first: cargo build --workspace"
    exit 1
fi

# Run main validation
main