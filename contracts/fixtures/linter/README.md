# Contract Linter Test Fixtures

This directory contains test fixtures for the contract linter CI job.

## Files

### Baseline
- `base-v1.0.0.json`: Base contract schema with name (string), age (integer), email (string)

### Compatible Change (PASS)
- `compatible-v1.1.0.json`: Adds optional `phone` field
- **Test**: `contract-linter compare --current base-v1.0.0.json --proposed compatible-v1.1.0.json --current-version 1.0.0 --proposed-version 1.1.0`
- **Expected**: Exit code 0 (success) - minor version bump valid for backward-compatible change

### Breaking Change Without Major Bump (FAIL)
- `breaking-v1.1.0.json`: Changes `age` type from integer to string
- **Test**: `contract-linter compare --current base-v1.0.0.json --proposed breaking-v1.1.0.json --current-version 1.0.0 --proposed-version 1.1.0`
- **Expected**: Exit code 1 (failure) - breaking change requires major version bump

### Breaking Change With Major Bump (PASS)
- `breaking-v2.0.0.json`: Changes `age` type from integer to string with major version bump
- **Test**: `contract-linter compare --current base-v1.0.0.json --proposed breaking-v2.0.0.json --current-version 1.0.0 --proposed-version 2.0.0`
- **Expected**: Exit code 0 (success) - major version bump allows breaking change

## Usage in CI

The CI workflow should run these tests to verify the linter correctly gates breaking changes:

```bash
# This should pass
cargo run -p contract-linter -- compare \
  --current contracts/fixtures/linter/base-v1.0.0.json \
  --proposed contracts/fixtures/linter/compatible-v1.1.0.json \
  --current-version 1.0.0 \
  --proposed-version 1.1.0

# This should fail
cargo run -p contract-linter -- compare \
  --current contracts/fixtures/linter/base-v1.0.0.json \
  --proposed contracts/fixtures/linter/breaking-v1.1.0.json \
  --current-version 1.0.0 \
  --proposed-version 1.1.0

# This should pass
cargo run -p contract-linter -- compare \
  --current contracts/fixtures/linter/base-v1.0.0.json \
  --proposed contracts/fixtures/linter/breaking-v2.0.0.json \
  --current-version 1.0.0 \
  --proposed-version 2.0.0
```
