#!/bin/bash

set -euo pipefail

# Documentation Link Checker
# Validates markdown links in documentation files using markdown-link-check

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DOCS_DIR="${PROJECT_ROOT}/docs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CHECK_EXTERNAL=false
QUIET=false
EXIT_ON_ERROR=true

usage() {
    cat << EOF
Usage: $0 [OPTIONS] [PATH]

Check markdown links in documentation files.

OPTIONS:
    -e, --external      Check external links (default: false)
    -q, --quiet         Suppress non-error output
    -c, --continue      Continue on errors (don't exit)
    -h, --help          Show this help message

ARGUMENTS:
    PATH               Path to check (default: docs/)

EXAMPLES:
    $0                               # Check all docs (internal links only)
    $0 --external                    # Check all docs including external links
    $0 docs/tutorials                # Check specific directory
    $0 --quiet docs/reference        # Check silently

DEPENDENCIES:
    - markdown-link-check: npm install -g markdown-link-check
    - find: Standard Unix command
EOF
}

log() {
    if [[ "$QUIET" != "true" ]]; then
        echo -e "$@"
    fi
}

error() {
    echo -e "${RED}ERROR:${NC} $*" >&2
}

warn() {
    echo -e "${YELLOW}WARNING:${NC} $*" >&2
}

success() {
    echo -e "${GREEN}SUCCESS:${NC} $*"
}

check_dependencies() {
    if ! command -v markdown-link-check &> /dev/null; then
        error "markdown-link-check is not installed"
        echo "Install it with: npm install -g markdown-link-check"
        exit 1
    fi
}

create_config() {
    local config_file="${PROJECT_ROOT}/.markdown-link-check.json"

    if [[ ! -f "$config_file" ]]; then
        log "Creating markdown-link-check configuration..."
        cat > "$config_file" << 'EOF'
{
  "ignorePatterns": [
    {
      "pattern": "^http://localhost"
    },
    {
      "pattern": "^https://localhost"
    },
    {
      "pattern": "^file://"
    }
  ],
  "replacementPatterns": [
    {
      "pattern": "^/",
      "replacement": "{{BASEURL}}/"
    }
  ],
  "httpHeaders": [
    {
      "urls": ["https://github.com"],
      "headers": {
        "Accept": "text/html"
      }
    }
  ],
  "timeout": "10s",
  "retryOn429": true,
  "retryCount": 3,
  "fallbackRetryDelay": "30s",
  "aliveStatusCodes": [200, 206]
}
EOF
    fi

    echo "$config_file"
}

check_links() {
    local target_path="$1"
    local config_file
    config_file=$(create_config)

    if [[ ! -e "$target_path" ]]; then
        error "Path does not exist: $target_path"
        return 1
    fi

    # Find all markdown files
    local markdown_files
    if [[ -d "$target_path" ]]; then
        markdown_files=$(find "$target_path" -name "*.md" -type f)
    else
        markdown_files="$target_path"
    fi

    if [[ -z "$markdown_files" ]]; then
        warn "No markdown files found in: $target_path"
        return 0
    fi

    local total_files=0
    local failed_files=0
    local total_links=0
    local failed_links=0

    log "Checking links in markdown files..."
    log "Configuration: $(basename "$config_file")"
    log "External links: ${CHECK_EXTERNAL}"
    echo

    while IFS= read -r file; do
        [[ -z "$file" ]] && continue

        total_files=$((total_files + 1))
        local relative_path="${file#${PROJECT_ROOT}/}"

        log "Checking: ${relative_path}"

        # Build markdown-link-check command
        local cmd_args=("--config" "$config_file")

        # Note: --disable-external flag was removed in markdown-link-check 3.x
        # External link checking is now controlled via ignorePatterns in config

        if [[ "$QUIET" == "true" ]]; then
            cmd_args+=("--quiet")
        fi

        # Run markdown-link-check and capture output
        local output
        local exit_code=0

        output=$(markdown-link-check "${cmd_args[@]}" "$file" 2>&1) || exit_code=$?

        # Parse output for statistics
        local file_links=0
        local file_errors=0

        if [[ -n "$output" ]]; then
            file_links=$(echo "$output" | grep -c "^\[" || true)
            file_errors=$(echo "$output" | grep -c "✖" || true)
        fi

        total_links=$((total_links + file_links))
        failed_links=$((failed_links + file_errors))

        if [[ $exit_code -ne 0 ]] || [[ $file_errors -gt 0 ]]; then
            failed_files=$((failed_files + 1))
            error "Link check failed for: $relative_path"
            if [[ "$QUIET" != "true" ]] && [[ -n "$output" ]]; then
                echo "$output" | grep "✖" || true
            fi
            echo

            if [[ "$EXIT_ON_ERROR" == "true" ]]; then
                return 1
            fi
        else
            if [[ "$QUIET" != "true" ]]; then
                success "All links valid in: $relative_path ($file_links links)"
            fi
        fi
    done <<< "$markdown_files"

    echo
    log "Link check summary:"
    log "  Files checked: $total_files"
    log "  Links checked: $total_links"
    log "  Failed files: $failed_files"
    log "  Failed links: $failed_links"

    if [[ $failed_files -eq 0 ]]; then
        success "All documentation links are valid!"
        return 0
    else
        error "$failed_files file(s) have broken links"
        return 1
    fi
}

main() {
    local target_path="$DOCS_DIR"

    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -e|--external)
                CHECK_EXTERNAL=true
                shift
                ;;
            -q|--quiet)
                QUIET=true
                shift
                ;;
            -c|--continue)
                EXIT_ON_ERROR=false
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            -*)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                target_path="$1"
                shift
                ;;
        esac
    done

    # Convert relative path to absolute
    if [[ ! "$target_path" = /* ]]; then
        target_path="${PROJECT_ROOT}/${target_path}"
    fi

    log "Documentation Link Checker"
    log "Project: $(basename "$PROJECT_ROOT")"
    log "Target: ${target_path#${PROJECT_ROOT}/}"
    echo

    check_dependencies
    check_links "$target_path"
}

# Run main function with all arguments
main "$@"