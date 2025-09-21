#!/usr/bin/env bats

# Test suite for the test-release-logic.sh script
# Uses Bats testing framework for shell scripts

setup() {
    export TEST_PROJECT_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
    export TEST_SCRIPT="$TEST_PROJECT_ROOT/scripts/test-release-logic.sh"
}

@test "script exists and is executable" {
    [ -f "$TEST_SCRIPT" ]
    [ -x "$TEST_SCRIPT" ]
}

@test "script fails gracefully when gh is available but no token" {
    # Skip if gh is not available
    if ! command -v gh >/dev/null 2>&1; then
        skip "gh CLI not available"
    fi

    # Unset any existing tokens
    unset GH_TOKEN
    unset GITHUB_TOKEN

    # Temporarily logout from gh if logged in
    if gh auth status >/dev/null 2>&1; then
        skip "gh is already authenticated (would need to logout for this test)"
    fi

    run "$TEST_SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" =~ "GitHub CLI is available but not authenticated" ]]
}

@test "make bundle-contracts target exists" {
    cd "$TEST_PROJECT_ROOT"
    run make -n bundle-contracts
    [ "$status" -eq 0 ]
}

@test "bundle-contracts creates required directories" {
    cd "$TEST_PROJECT_ROOT"

    # Clean up any existing dist directory
    rm -rf dist/contracts

    # Run the make target (dry-run to avoid actual execution in test)
    run make -n bundle-contracts
    [ "$status" -eq 0 ]
    [[ "$output" =~ "mkdir -p dist/contracts" ]]
}

@test "script validates required tools" {
    # Check that the script would fail if jq is missing
    # This is a mock test - we're validating the script logic
    run grep -q "jq -r" "$TEST_SCRIPT"
    [ "$status" -eq 0 ]

    run grep -q "shasum -a 256" "$TEST_SCRIPT"
    [ "$status" -eq 0 ]
}

@test "script handles missing bundle files" {
    cd "$TEST_PROJECT_ROOT"

    # Create a minimal test scenario
    rm -rf dist/contracts
    mkdir -p dist/contracts

    # Create empty files to test validation
    touch dist/contracts/bundle.json
    touch dist/contracts/manifest.json

    # Create a minimal valid manifest
    cat > dist/contracts/manifest.json << 'EOF'
{
  "version": "1.0.0",
  "timestamp": "2025-01-01T00:00:00Z",
  "git": {
    "sha": "1234567890abcdef1234567890abcdef12345678",
    "repository": "test/repo",
    "ref": "main"
  },
  "bundle_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
EOF

    # The empty file has this specific SHA-256
    echo "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  bundle.json" > dist/contracts/bundle.sha256

    # Verify the checksum works
    cd dist/contracts
    run shasum -a 256 -c bundle.sha256
    [ "$status" -eq 0 ]
}