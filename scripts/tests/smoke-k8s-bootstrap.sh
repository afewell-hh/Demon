#!/bin/bash
#
# End-to-End Smoke Test for Kubernetes Bootstrapper
# Automates cluster provisioning, bootstrap deployment, and verification
#
# This script validates the full Kubernetes bootstrapper deployment by performing
# HTTP health checks against the deployed Demon components (runtime, engine, operate-ui)
# to ensure they are responding correctly to requests.
#
set -euo pipefail

# Default configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
ARTIFACTS_DIR="${PROJECT_ROOT}/dist/bootstrapper-smoke/${TIMESTAMP}"
CONFIG_FILE="${PROJECT_ROOT}/scripts/tests/fixtures/config.e2e.yaml"
CLUSTER_NAME="demon-smoke-test"
TIMEOUT_SECONDS=300
POLL_INTERVAL=10
DRY_RUN=${DRY_RUN:-false}
CLEANUP=${CLEANUP:-false}
VERBOSE=${VERBOSE:-false}

# Tools configuration - k3d preferred, kind as fallback
CONTAINER_TOOL=""
KUBECTL_CMD=""

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $*"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $*" >&2
}

error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

End-to-End smoke test for the Kubernetes bootstrapper.

OPTIONS:
    --dry-run-only      Skip live cluster creation, only validate config and templates
    --cleanup           Tear down cluster after test completion
    --verbose           Enable verbose output
    --config FILE       Use custom config file (default: scripts/tests/fixtures/config.e2e.yaml)
    --timeout SECONDS   Pod readiness timeout in seconds (default: 300)
    --artifacts-dir DIR Custom directory for artifacts (default: dist/bootstrapper-smoke/<timestamp>)
    --help              Show this help message

ENVIRONMENT VARIABLES:
    DRY_RUN=1          Same as --dry-run-only
    CLEANUP=1          Same as --cleanup
    VERBOSE=1          Same as --verbose

EXAMPLES:
    # Full smoke test with cleanup
    $0 --cleanup

    # Dry-run validation only
    $0 --dry-run-only

    # Custom config with verbose output
    $0 --config ./my-config.yaml --verbose

    # Quick validation via environment
    DRY_RUN=1 $0

EOF
}

check_dependencies() {
    log "Checking dependencies..."

    # Check for demonctl
    if ! command -v "${PROJECT_ROOT}/target/debug/demonctl" >/dev/null 2>&1; then
        if ! cargo build -p demonctl >/dev/null 2>&1; then
            error "Failed to build demonctl. Run 'cargo build -p demonctl' first."
            exit 1
        fi
    fi

    # Check for container runtime tools
    if command -v k3d >/dev/null 2>&1; then
        CONTAINER_TOOL="k3d"
        KUBECTL_CMD="k3d kubeconfig write ${CLUSTER_NAME} --output -"
        log "Found k3d (preferred)"
    elif command -v kind >/dev/null 2>&1; then
        CONTAINER_TOOL="kind"
        KUBECTL_CMD="kind get kubeconfig --name ${CLUSTER_NAME}"
        log "Found kind (fallback)"
    else
        error "Neither k3d nor kind found. Please install one of:"
        error "  k3d: https://k3d.io/v5.4.6/#installation"
        error "  kind: https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
        exit 1
    fi

    # Check for kubectl
    if ! command -v kubectl >/dev/null 2>&1; then
        error "kubectl not found. Please install kubectl."
        exit 1
    fi

    # Check for docker/podman
    if ! command -v docker >/dev/null 2>&1 && ! command -v podman >/dev/null 2>&1; then
        error "Neither docker nor podman found. Container runtime required for ${CONTAINER_TOOL}."
        exit 1
    fi

    success "All dependencies available"
}

create_artifacts_dir() {
    log "Creating artifacts directory: ${ARTIFACTS_DIR}"
    mkdir -p "${ARTIFACTS_DIR}"
}

provision_cluster() {
    if [[ "${DRY_RUN}" == "true" ]]; then
        log "Dry-run mode: skipping cluster provisioning"
        return 0
    fi

    log "Provisioning ${CONTAINER_TOOL} cluster: ${CLUSTER_NAME}"

    case "${CONTAINER_TOOL}" in
        "k3d")
            # Create k3d cluster
            if k3d cluster list | grep -q "${CLUSTER_NAME}"; then
                warn "Cluster ${CLUSTER_NAME} already exists, deleting..."
                k3d cluster delete "${CLUSTER_NAME}"
            fi

            k3d cluster create "${CLUSTER_NAME}" \
                --agents 0 \
                --wait \
                --timeout 60s \
                --registry-create demon-registry:0.0.0.0:5000
            ;;
        "kind")
            # Create kind cluster
            if kind get clusters | grep -q "${CLUSTER_NAME}"; then
                warn "Cluster ${CLUSTER_NAME} already exists, deleting..."
                kind delete cluster --name "${CLUSTER_NAME}"
            fi

            cat > "${ARTIFACTS_DIR}/kind-config.yaml" << EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 30080
    hostPort: 30080
    protocol: TCP
EOF

            kind create cluster --name "${CLUSTER_NAME}" --config "${ARTIFACTS_DIR}/kind-config.yaml" --wait 60s
            ;;
    esac

    # Export kubeconfig for kubectl usage
    ${KUBECTL_CMD} > "${ARTIFACTS_DIR}/kubeconfig"
    export KUBECONFIG="${ARTIFACTS_DIR}/kubeconfig"

    # Verify cluster readiness
    log "Waiting for cluster nodes to be ready..."
    kubectl wait --for=condition=Ready nodes --all --timeout=60s

    success "Cluster ${CLUSTER_NAME} is ready"

    # Capture cluster info
    kubectl cluster-info > "${ARTIFACTS_DIR}/cluster-info.txt" 2>&1
    kubectl get nodes -o wide > "${ARTIFACTS_DIR}/nodes.txt" 2>&1
}

run_dry_run_bootstrap() {
    log "Running dry-run bootstrap validation..."

    local output_file="${ARTIFACTS_DIR}/dry-run-output.txt"
    local manifest_file="${ARTIFACTS_DIR}/dry-run-manifests.yaml"

    cd "${PROJECT_ROOT}"

    # Capture dry-run output
    if [[ "${VERBOSE}" == "true" ]]; then
        "${PROJECT_ROOT}/target/debug/demonctl" k8s-bootstrap bootstrap \
            --config "${CONFIG_FILE}" \
            --dry-run \
            --verbose > "${output_file}" 2>&1
    else
        "${PROJECT_ROOT}/target/debug/demonctl" k8s-bootstrap bootstrap \
            --config "${CONFIG_FILE}" \
            --dry-run > "${output_file}" 2>&1
    fi

    # Extract manifests from verbose output if available
    if [[ "${VERBOSE}" == "true" ]] && grep -q "Generated manifests:" "${output_file}"; then
        awk '/Generated manifests:/,EOF {print}' "${output_file}" | tail -n +3 > "${manifest_file}"
    fi

    success "Dry-run validation completed"

    # Validate output contains expected success indicators
    if ! grep -q "Configuration is valid" "${output_file}"; then
        error "Dry-run did not report valid configuration"
        cat "${output_file}"
        exit 1
    fi
}

run_live_bootstrap() {
    if [[ "${DRY_RUN}" == "true" ]]; then
        log "Dry-run mode: skipping live bootstrap"
        return 0
    fi

    log "Running live bootstrap deployment..."

    local output_file="${ARTIFACTS_DIR}/bootstrap-output.txt"

    cd "${PROJECT_ROOT}"
    export KUBECONFIG="${ARTIFACTS_DIR}/kubeconfig"

    # Set required environment variables for the test
    export GITHUB_TOKEN="${GITHUB_TOKEN:-test-token-value}"
    export ADMIN_TOKEN="${ADMIN_TOKEN:-test-admin-token}"
    export DATABASE_URL="${DATABASE_URL:-postgresql://localhost/demon_test}"
    export NATS_PASSWORD="${NATS_PASSWORD:-test-nats-password}"
    export JWT_SECRET="${JWT_SECRET:-test-jwt-secret-12345}"

    # Run bootstrap with verbose output
    if [[ "${VERBOSE}" == "true" ]]; then
        "${PROJECT_ROOT}/target/debug/demonctl" k8s-bootstrap bootstrap \
            --config "${CONFIG_FILE}" \
            --verbose > "${output_file}" 2>&1
    else
        "${PROJECT_ROOT}/target/debug/demonctl" k8s-bootstrap bootstrap \
            --config "${CONFIG_FILE}" > "${output_file}" 2>&1
    fi

    success "Bootstrap deployment completed"
}

validate_deployment() {
    if [[ "${DRY_RUN}" == "true" ]]; then
        log "Dry-run mode: skipping deployment validation"
        return 0
    fi

    log "Validating deployment..."

    export KUBECONFIG="${ARTIFACTS_DIR}/kubeconfig"
    local namespace="demon-system"

    # Wait for pods to be ready
    log "Waiting for pods in namespace ${namespace} to be ready (timeout: ${TIMEOUT_SECONDS}s)..."

    local elapsed=0
    while [[ ${elapsed} -lt ${TIMEOUT_SECONDS} ]]; do
        # Get pod status
        kubectl get pods -n "${namespace}" --no-headers > "${ARTIFACTS_DIR}/pods-status.txt" 2>/dev/null || true

        if [[ -s "${ARTIFACTS_DIR}/pods-status.txt" ]]; then
            local total_pods
            total_pods=$(wc -l < "${ARTIFACTS_DIR}/pods-status.txt")

            local ready_pods
            ready_pods=$(awk '$3 == "Running" && $2 ~ /^[0-9]+\/[0-9]+$/ {
                split($2, a, "/");
                if (a[1] == a[2]) print
            }' "${ARTIFACTS_DIR}/pods-status.txt" | wc -l)

            if [[ "${VERBOSE}" == "true" ]]; then
                log "Pods ready: ${ready_pods}/${total_pods}"
            fi

            if [[ ${ready_pods} -eq ${total_pods} && ${total_pods} -gt 0 ]]; then
                success "All ${total_pods} pods are ready"
                break
            fi
        else
            if [[ "${VERBOSE}" == "true" ]]; then
                log "No pods found yet, waiting..."
            fi
        fi

        sleep ${POLL_INTERVAL}
        elapsed=$((elapsed + POLL_INTERVAL))
    done

    if [[ ${elapsed} -ge ${TIMEOUT_SECONDS} ]]; then
        error "Timeout waiting for pods to be ready"
        kubectl get pods -n "${namespace}" -o wide || true
        exit 1
    fi

    # Verify secrets exist
    log "Checking for demon-secrets..."
    if ! kubectl get secret demon-secrets -n "${namespace}" > "${ARTIFACTS_DIR}/secret-info.txt" 2>&1; then
        error "demon-secrets not found"
        exit 1
    fi
    success "demon-secrets found"

    # Verify services are accessible
    log "Checking services..."
    kubectl get services -n "${namespace}" > "${ARTIFACTS_DIR}/services.txt" 2>&1

    # Run integrated health checks
    run_health_checks "${namespace}"
}

run_health_checks() {
    local namespace="$1"
    log "Running health checks..."

    # Check runtime pod health endpoint
    local runtime_pod
    runtime_pod=$(kubectl get pods -n "${namespace}" -l app.kubernetes.io/name=demon-runtime -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")

    if [[ -n "${runtime_pod}" ]]; then
        log "Checking runtime health endpoint: ${runtime_pod}"

        if kubectl exec -n "${namespace}" "${runtime_pod}" -- wget -q -O- http://localhost:8080/health >/dev/null 2>&1; then
            success "Runtime health check passed"
            kubectl exec -n "${namespace}" "${runtime_pod}" -- wget -q -O- http://localhost:8080/health > "${ARTIFACTS_DIR}/runtime-health.txt" 2>&1 || true
        else
            error "Runtime health check failed"
            kubectl logs -n "${namespace}" "${runtime_pod}" > "${ARTIFACTS_DIR}/runtime-health-logs.txt" 2>&1 || true
            return 1
        fi
    else
        error "No demon-runtime pod found for health checking"
        return 1
    fi

    # Check Operate UI pod health endpoint
    local ui_pod
    ui_pod=$(kubectl get pods -n "${namespace}" -l app.kubernetes.io/name=operate-ui -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")

    if [[ -n "${ui_pod}" ]]; then
        log "Checking Operate UI health endpoint: ${ui_pod}"

        if kubectl exec -n "${namespace}" "${ui_pod}" -- wget -q -O- http://localhost:3000/api/health >/dev/null 2>&1; then
            success "Operate UI health check passed"
            kubectl exec -n "${namespace}" "${ui_pod}" -- wget -q -O- http://localhost:3000/api/health > "${ARTIFACTS_DIR}/ui-health.txt" 2>&1 || true
        else
            error "Operate UI health check failed"
            kubectl logs -n "${namespace}" "${ui_pod}" > "${ARTIFACTS_DIR}/ui-health-logs.txt" 2>&1 || true
            return 1
        fi
    else
        error "No operate-ui pod found for health checking"
        return 1
    fi

    # Check Engine pod health endpoint
    local engine_pod
    engine_pod=$(kubectl get pods -n "${namespace}" -l app.kubernetes.io/name=demon-engine -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")

    if [[ -n "${engine_pod}" ]]; then
        log "Checking Engine health endpoint: ${engine_pod}"

        if kubectl exec -n "${namespace}" "${engine_pod}" -- wget -q -O- http://localhost:8081/health >/dev/null 2>&1; then
            success "Engine health check passed"
            kubectl exec -n "${namespace}" "${engine_pod}" -- wget -q -O- http://localhost:8081/health > "${ARTIFACTS_DIR}/engine-health.txt" 2>&1 || true
        else
            error "Engine health check failed"
            kubectl logs -n "${namespace}" "${engine_pod}" > "${ARTIFACTS_DIR}/engine-health-logs.txt" 2>&1 || true
            return 1
        fi
    else
        error "No demon-engine pod found for health checking"
        return 1
    fi

    success "All health checks passed!"
}

capture_logs() {
    if [[ "${DRY_RUN}" == "true" ]]; then
        log "Dry-run mode: skipping log capture"
        return 0
    fi

    log "Capturing cluster logs and state..."

    export KUBECONFIG="${ARTIFACTS_DIR}/kubeconfig"
    local namespace="demon-system"

    # Create artifacts subdirectory structure
    mkdir -p "${ARTIFACTS_DIR}/manifests"
    mkdir -p "${ARTIFACTS_DIR}/logs"
    mkdir -p "${ARTIFACTS_DIR}/descriptions"

    # Capture all resources in the namespace
    log "Capturing resource manifests..."
    kubectl get all -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/all-resources.yaml" 2>&1 || true
    kubectl get pods -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/pods.yaml" 2>&1 || true
    kubectl get services -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/services.yaml" 2>&1 || true
    kubectl get secrets -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/secrets.yaml" 2>&1 || true
    kubectl get configmaps -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/configmaps.yaml" 2>&1 || true

    # Capture ingress if enabled
    if kubectl get ingress -n "${namespace}" --no-headers 2>/dev/null | grep -q .; then
        kubectl get ingress -n "${namespace}" -o yaml > "${ARTIFACTS_DIR}/manifests/ingress.yaml" 2>&1 || true
    fi

    # Capture pod descriptions
    log "Capturing resource descriptions..."
    kubectl describe pods -n "${namespace}" > "${ARTIFACTS_DIR}/descriptions/pods-describe.txt" 2>&1 || true
    kubectl describe services -n "${namespace}" > "${ARTIFACTS_DIR}/descriptions/services-describe.txt" 2>&1 || true
    kubectl describe secrets -n "${namespace}" > "${ARTIFACTS_DIR}/descriptions/secrets-describe.txt" 2>&1 || true

    # Capture logs from each pod
    log "Capturing pod logs..."
    local pods
    pods=$(kubectl get pods -n "${namespace}" -o jsonpath='{.items[*].metadata.name}' 2>/dev/null || echo "")

    if [[ -n "${pods}" ]]; then
        for pod in ${pods}; do
            log "Capturing logs for pod: ${pod}"
            kubectl logs -n "${namespace}" "${pod}" > "${ARTIFACTS_DIR}/logs/${pod}.txt" 2>&1 || true

            # Capture previous logs if pod has restarted
            if kubectl logs -n "${namespace}" "${pod}" --previous >/dev/null 2>&1; then
                kubectl logs -n "${namespace}" "${pod}" --previous > "${ARTIFACTS_DIR}/logs/${pod}-previous.txt" 2>&1 || true
            fi
        done
    fi

    # Capture cluster events
    log "Capturing cluster events..."
    kubectl get events -n "${namespace}" --sort-by='.lastTimestamp' > "${ARTIFACTS_DIR}/events.txt" 2>&1 || true
    kubectl get events --all-namespaces --sort-by='.lastTimestamp' > "${ARTIFACTS_DIR}/events-all-namespaces.txt" 2>&1 || true

    # Final cluster state summary
    log "Capturing final cluster state..."
    kubectl get all -n "${namespace}" -o wide > "${ARTIFACTS_DIR}/final-state.txt" 2>&1 || true
    kubectl get nodes -o wide > "${ARTIFACTS_DIR}/nodes-final.txt" 2>&1 || true

    # Capture cluster info for CI/automation
    log "Capturing cluster information..."
    kubectl version --output=yaml > "${ARTIFACTS_DIR}/cluster-version.txt" 2>&1 || true
    kubectl cluster-info dump --namespaces="${namespace}" --output-directory="${ARTIFACTS_DIR}/cluster-dump" 2>&1 || true

    success "Comprehensive artifacts collected in ${ARTIFACTS_DIR}"
    log "Artifact structure:"
    log "  - manifests/: YAML exports of all resources"
    log "  - logs/: Pod logs (current and previous if available)"
    log "  - descriptions/: Detailed resource descriptions"
    log "  - events.txt: Kubernetes events in target namespace"
    log "  - final-state.txt: Final resource summary"
}

cleanup_cluster() {
    if [[ "${CLEANUP}" != "true" ]]; then
        log "Cleanup not requested (use --cleanup to tear down cluster)"
        return 0
    fi

    if [[ "${DRY_RUN}" == "true" ]]; then
        log "Dry-run mode: skipping cleanup"
        return 0
    fi

    log "Cleaning up cluster ${CLUSTER_NAME}..."

    case "${CONTAINER_TOOL}" in
        "k3d")
            k3d cluster delete "${CLUSTER_NAME}" || true
            ;;
        "kind")
            kind delete cluster --name "${CLUSTER_NAME}" || true
            ;;
    esac

    success "Cluster cleanup completed"
}

print_summary() {
    echo
    echo "=================================================================="
    echo "                    SMOKE TEST SUMMARY"
    echo "=================================================================="
    echo "Timestamp: ${TIMESTAMP}"
    echo "Artifacts: ${ARTIFACTS_DIR}"
    echo "Config: ${CONFIG_FILE}"
    echo "Cluster Tool: ${CONTAINER_TOOL}"
    echo "Dry Run: ${DRY_RUN}"
    echo "Cleanup: ${CLEANUP}"
    echo

    if [[ "${DRY_RUN}" == "true" ]]; then
        echo "✓ Configuration validation passed"
        echo "✓ Template rendering completed"
        echo "✓ Dry-run artifacts captured"
    else
        echo "✓ Cluster provisioning completed"
        echo "✓ Bootstrap deployment completed"
        echo "✓ Pod readiness verified"
        echo "✓ Health checks passed"
        echo "✓ Comprehensive artifacts captured"
        if [[ "${CLEANUP}" == "true" ]]; then
            echo "✓ Cluster cleanup completed"
        else
            echo "ℹ Cluster left running for manual inspection"
            echo "  To access: export KUBECONFIG=${ARTIFACTS_DIR}/kubeconfig"
            echo "  To cleanup: ${CONTAINER_TOOL} cluster delete ${CLUSTER_NAME}"
        fi
    fi

    echo
    echo "Key artifacts:"
    if [[ "${DRY_RUN}" == "true" ]]; then
        echo "  - ${ARTIFACTS_DIR}/dry-run-output.txt"
        [[ -f "${ARTIFACTS_DIR}/dry-run-manifests.yaml" ]] && echo "  - ${ARTIFACTS_DIR}/dry-run-manifests.yaml"
    else
        echo "  - ${ARTIFACTS_DIR}/kubeconfig"
        echo "  - ${ARTIFACTS_DIR}/bootstrap-output.txt"
        echo "  - ${ARTIFACTS_DIR}/manifests/ (resource YAML exports)"
        echo "  - ${ARTIFACTS_DIR}/logs/ (pod logs)"
        echo "  - ${ARTIFACTS_DIR}/descriptions/ (detailed resource info)"
        echo "  - ${ARTIFACTS_DIR}/runtime-health.txt & ui-health.txt"
        echo "  - ${ARTIFACTS_DIR}/final-state.txt"
    fi
    echo "=================================================================="
}

main() {
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run-only)
                DRY_RUN=true
                shift
                ;;
            --cleanup)
                CLEANUP=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            --timeout)
                TIMEOUT_SECONDS="$2"
                shift 2
                ;;
            --artifacts-dir)
                ARTIFACTS_DIR="$2"
                shift 2
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done

    # Validate config file exists
    if [[ ! -f "${CONFIG_FILE}" ]]; then
        error "Config file not found: ${CONFIG_FILE}"
        exit 1
    fi

    # Set up trap for cleanup on exit
    trap 'cleanup_cluster; exit' INT TERM

    log "Starting Kubernetes bootstrapper smoke test..."
    log "Mode: $(if [[ "${DRY_RUN}" == "true" ]]; then echo "DRY-RUN"; else echo "FULL"; fi)"

    check_dependencies
    create_artifacts_dir

    if [[ "${DRY_RUN}" == "true" ]]; then
        run_dry_run_bootstrap
    else
        provision_cluster
        run_dry_run_bootstrap  # Always validate first
        run_live_bootstrap
        validate_deployment
    fi

    capture_logs
    print_summary

    success "Smoke test completed successfully!"
}

# Run main function
main "$@"