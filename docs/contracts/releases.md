# Contract Bundle Releases

This document describes how Demon contract bundles are published to GitHub Releases for downstream consumption.

## Overview

Contract bundles are automatically published to GitHub Releases whenever CI completes successfully on the `main` branch. This provides a stable, versioned distribution channel for consumers who need to fetch contract schemas and WIT definitions.

## Release Process

### Automatic Publishing

1. When CI runs on `main` and the `contract-bundle` job succeeds, the `contracts-release` workflow triggers
2. The workflow downloads the contract bundle artifacts produced by CI
3. A release is created with a timestamp-based tag: `contracts-YYYYMMDD-shortsha`
4. The release includes all bundle files with integrity verification

### Release Contents

Each release contains:

- **`bundle.json`** - Contract schemas and WIT definitions
- **`manifest.json`** - Bundle metadata with integrity hash
- **`bundle.sha256`** - SHA-256 checksum for verification

### Release Naming

- **Versioned releases**: `contracts-20250920-4c99ca47` (timestamp + short SHA)
- **Latest alias**: `contracts-latest` (always points to the most recent release)

## Downloading Releases

### Using demonctl (Recommended)

```bash
# Download latest bundle
demonctl contracts fetch-bundle

# Download specific release
demonctl contracts fetch-bundle --release contracts-20250920-4c99ca47
```

### Using GitHub CLI

```bash
# Download latest release files
gh release download contracts-latest -p "*.json" -p "*.sha256"

# Download specific release
gh release download contracts-20250920-4c99ca47 -p "*.json" -p "*.sha256"

# List all contract releases
gh release list --limit 20 | grep "contracts-"
```

### Direct Download

Visit the [releases page](https://github.com/afewell-hh/demon/releases) and download the files manually.

## Verification

Always verify bundle integrity after downloading:

```bash
# Verify using provided checksum
shasum -a 256 -c bundle.sha256

# Or verify manually
shasum -a 256 bundle.json
# Compare with manifest.json bundle_sha256 field
```

## Release Metadata

Each release includes structured metadata in `manifest.json`:

```json
{
  "version": "1.0.0",
  "timestamp": "2025-09-20T18:11:29Z",
  "git": {
    "sha": "4c99ca4745d13fb1277f1a60cd7028ee4e39bb34",
    "branch": "main"
  },
  "bundle": "bundle.json",
  "bundle_sha256": "e7b789d169817c54151f128e666adb53d33bf2200c89ab9f28869d5c2a7a2052",
  "description": "Demon contract schemas and WIT definitions"
}
```

## Workflow Details

The release workflow (`.github/workflows/contracts-release.yml`) includes:

- **Integrity verification**: Recomputes and validates SHA-256 checksums
- **Idempotent releases**: Safe to re-run; updates existing releases if needed
- **Latest alias management**: Maintains `contracts-latest` for convenience
- **Comprehensive metadata**: Includes git SHA, timestamp, and verification details

## For Maintainers

### Testing Release Logic

Run the test script to validate release logic locally:

```bash
./scripts/test-release-logic.sh
```

This script:
- Generates a contract bundle
- Extracts and validates metadata
- Creates checksum files
- Verifies integrity
- Simulates release creation steps

### Manual Release Creation

If needed, you can manually trigger a release:

```bash
# Generate bundle
make bundle-contracts

# Run release script logic manually
# (extract from .github/workflows/contracts-release.yml)
```

### Troubleshooting

Common issues:

- **SHA-256 mismatch**: Bundle was corrupted during transfer
- **Release already exists**: Workflow updates existing releases safely
- **Permission denied**: Ensure `GITHUB_TOKEN` has `contents: write` permission

## Integration Examples

### CI/CD Pipeline Integration

```yaml
steps:
  - name: Download latest contracts
    run: |
      gh release download contracts-latest -p "bundle.json" -p "bundle.sha256"
      shasum -a 256 -c bundle.sha256
```

### Application Integration

```bash
#!/bin/bash
# Download and verify contracts in your application
set -euo pipefail

# Download latest
gh release download contracts-latest -p "bundle.json" -p "manifest.json" -p "bundle.sha256"

# Verify integrity
if ! shasum -a 256 -c bundle.sha256; then
    echo "Contract bundle verification failed!"
    exit 1
fi

# Extract version for logging
VERSION=$(jq -r '.version' manifest.json)
echo "Using contract bundle version: $VERSION"
```