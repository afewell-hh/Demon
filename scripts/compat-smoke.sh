#!/usr/bin/env bash
# Compatibility smoke test for Demon API versioning
#
# This script tests API compatibility by verifying:
# 1. Version headers are present on all API responses
# 2. Unsupported versions return 406 Not Acceptable
# 3. No version header defaults to v1
# 4. All versioned endpoints respond correctly

set -euo pipefail

# Configuration
BASE_URL="${DEMON_API_URL:-http://localhost:3000}"
EXPECTED_VERSION="v1"
TIMEOUT=5

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

test_endpoint() {
    local method="$1"
    local endpoint="$2"
    local version_header="$3"
    local expected_status="$4"
    local test_name="$5"

    TESTS_RUN=$((TESTS_RUN + 1))

    log_info "Test ${TESTS_RUN}: ${test_name}"

    # Build curl command
    local curl_cmd="curl -s -w '\\n%{http_code}\\n%{header_json}' --max-time ${TIMEOUT}"

    if [[ -n "$version_header" ]]; then
        curl_cmd="$curl_cmd -H 'X-Demon-API-Version: $version_header'"
    fi

    if [[ "$method" == "POST" ]]; then
        curl_cmd="$curl_cmd -X POST -H 'Content-Type: application/json' -d '{}'"
    fi

    curl_cmd="$curl_cmd '${BASE_URL}${endpoint}'"

    # Execute request
    local response
    if ! response=$(eval "$curl_cmd" 2>&1); then
        log_error "  ✗ Request failed: $response"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    # Parse response (last two lines are status code and headers)
    local body=$(echo "$response" | head -n -2)
    local status_code=$(echo "$response" | tail -n 2 | head -n 1)
    local headers=$(echo "$response" | tail -n 1)

    # Check status code
    if [[ "$status_code" != "$expected_status" ]]; then
        log_error "  ✗ Expected status $expected_status, got $status_code"
        log_error "  Response: $body"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    # Check version header (for API endpoints only)
    if [[ "$endpoint" == /api/* ]]; then
        local version_header_value=$(echo "$headers" | jq -r '.["x-demon-api-version"][0] // empty' 2>/dev/null || echo "")

        if [[ -z "$version_header_value" ]]; then
            log_error "  ✗ Missing X-Demon-API-Version header"
            log_error "  Headers: $headers"
            TESTS_FAILED=$((TESTS_FAILED + 1))
            return 1
        fi

        if [[ "$version_header_value" != "$EXPECTED_VERSION" ]]; then
            log_error "  ✗ Expected version header $EXPECTED_VERSION, got $version_header_value"
            TESTS_FAILED=$((TESTS_FAILED + 1))
            return 1
        fi
    fi

    log_info "  ✓ Passed (status: $status_code)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
    return 0
}

main() {
    log_info "=== Demon API Compatibility Smoke Test ==="
    log_info "Base URL: $BASE_URL"
    log_info "Expected API version: $EXPECTED_VERSION"
    log_info ""

    # Check if server is reachable
    if ! curl -s --max-time "$TIMEOUT" "${BASE_URL}/health" > /dev/null 2>&1; then
        log_error "Server is not reachable at $BASE_URL"
        log_error "Make sure the Demon operate-ui is running"
        exit 1
    fi

    log_info "Server is reachable, starting tests..."
    echo ""

    # Test 1: /api/runs with v1 header should return 200
    test_endpoint "GET" "/api/runs" "v1" "200" "GET /api/runs with v1 header"

    # Test 2: /api/runs without version header should return 200 (backwards compatible)
    test_endpoint "GET" "/api/runs" "" "200" "GET /api/runs without version header (backwards compat)"

    # Test 3: /api/runs with unsupported version should return 406
    test_endpoint "GET" "/api/runs" "v99" "406" "GET /api/runs with unsupported version"

    # Test 4: /api/contracts/status with v1 header should return 200
    test_endpoint "GET" "/api/contracts/status" "v1" "200" "GET /api/contracts/status with v1 header"

    # Test 5: /api/contracts/status without version header should return 200
    test_endpoint "GET" "/api/contracts/status" "" "200" "GET /api/contracts/status without version header"

    # Test 6: /api/contracts/status with unsupported version should return 406
    test_endpoint "GET" "/api/contracts/status" "v99" "406" "GET /api/contracts/status with unsupported version"

    # Test 7: POST /api/contracts/validate/envelope with v1 header
    test_endpoint "POST" "/api/contracts/validate/envelope" "v1" "200" "POST /api/contracts/validate/envelope with v1 header"

    # Test 8: POST /api/contracts/validate/envelope without version header
    test_endpoint "POST" "/api/contracts/validate/envelope" "" "200" "POST /api/contracts/validate/envelope without version header"

    # Test 9: POST /api/contracts/validate/envelope with unsupported version
    test_endpoint "POST" "/api/contracts/validate/envelope" "v99" "406" "POST /api/contracts/validate/envelope with unsupported version"

    echo ""
    log_info "=== Test Summary ==="
    log_info "Tests run:    $TESTS_RUN"
    log_info "Tests passed: $TESTS_PASSED"
    log_info "Tests failed: $TESTS_FAILED"
    echo ""

    if [[ $TESTS_FAILED -eq 0 ]]; then
        log_info "✓ All compatibility tests passed!"
        exit 0
    else
        log_error "✗ Some compatibility tests failed"
        exit 1
    fi
}

# Run main function
main "$@"
