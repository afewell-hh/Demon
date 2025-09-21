# Contract Bundle Operations Playbook

This runbook provides step-by-step operational guidance for managing Demon contract bundles in production environments.

## Overview

Contract bundles are automatically published via CI and distributed through GitHub Releases. This playbook covers manual operations, troubleshooting, and monitoring for operators managing contract bundle infrastructure.

## Release Cadence & Automation

### Automated Release Schedule

Contract bundles are released automatically via three triggers:

1. **CI-Triggered (Primary)**: Automatic release when main CI completes successfully
2. **Scheduled**: Daily at 6 AM UTC via cron schedule
3. **Manual**: On-demand via workflow dispatch

### Triggering a Manual Release

```bash
# Option 1: Via GitHub CLI (recommended)
gh workflow run contracts-release.yml

# Option 2: Force release with changes detection bypass
gh workflow run contracts-release.yml -f force_release=true

# Option 3: Via API
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/afewell-hh/demon/actions/workflows/contracts-release.yml/dispatches \
  -d '{"ref":"main","inputs":{"force_release":"false"}}'

# Monitor workflow progress
gh run list --workflow=contracts-release.yml --limit 5
```

### Scheduled Release Monitoring

```bash
# Check recent scheduled releases
gh run list --workflow=contracts-release.yml --event=schedule --limit 5

# Monitor today's scheduled release
TODAY=$(date +%Y-%m-%d)
gh run list --workflow=contracts-release.yml --created="$TODAY" | grep schedule || echo "No scheduled release today"
```

### Validating a Released Bundle

Use the automated validation script for comprehensive release verification:

```bash
# Validate latest release (recommended)
./scripts/validate-release.sh

# Validate specific release
./scripts/validate-release.sh contracts-20250921-0658fb8b

# Validate with verbose output
./scripts/validate-release.sh --verbose contracts-latest

# Validate using compiled binary (faster)
DEMONCTL_BIN=./target/release/demonctl ./scripts/validate-release.sh
```

**Manual validation steps** (if script not available):

```bash
# Download and verify latest release
demonctl contracts fetch-bundle --tag contracts-latest --dest ./validation
cd validation

# Verify integrity
shasum -a 256 -c bundle.sha256

# Validate bundle structure
demonctl contracts validate bundle.json

# Check manifest metadata
jq '.' manifest.json

# Verify git SHA and timestamp are recent
jq '.git.sha, .timestamp' manifest.json
```

### Testing Bundle Integration

```bash
# Test bundle ingestion at runtime
export DEMON_CONTRACTS_TAG=contracts-latest
export RUST_LOG=runtime::audit=info

# Clear cache to force fresh download
rm -rf .demon/contracts/

# Start runtime and monitor bundle loading
cargo run -p demonctl -- bootstrap | jq -r 'select(.event | startswith("bundle."))'

# Expected events: bundle.refresh_attempt, bundle.loaded
# Check bundle status via API
curl localhost:3000/api/contracts/status | jq '.contractBundle'
```

## Incident Response Procedures

### Bundle Download Failures

**Symptoms:**
- API returns `{"contractBundle": {"status": "download_error"}}`
- Logs show `bundle.download_failed` events
- Runtime falls back to embedded schemas

**Diagnosis:**
```bash
# Check GitHub release exists
gh release list --limit 10 | grep contracts-

# Test manual download
demonctl contracts fetch-bundle --dry-run --tag contracts-latest

# Check network connectivity
curl -I https://github.com/afewell-hh/demon/releases/latest

# Verify GitHub token (for private repos)
gh auth status
```

**Resolution:**
```bash
# Solution 1: Download fresh bundle manually
demonctl contracts fetch-bundle --tag contracts-latest

# Solution 2: Use specific known-good release
export DEMON_CONTRACTS_TAG=contracts-20250920-4c99ca47
systemctl restart demon-runtime

# Solution 3: Temporarily disable bundle loading
export DEMON_SKIP_CONTRACT_BUNDLE=1
systemctl restart demon-runtime
```

### Bundle Verification Failures

**Symptoms:**
- `bundle.verification_failed` audit events
- Alert: "Bundle verification failed"
- SHA-256 mismatch errors

**Diagnosis:**
```bash
# Check bundle integrity
cd .demon/contracts/
shasum -a 256 bundle.json
jq -r '.bundle_sha256' manifest.json

# Compare checksums - they should match
# If different, bundle was corrupted during transfer
```

**Resolution:**
```bash
# Solution 1: Re-download bundle
rm -rf .demon/contracts/
demonctl contracts fetch-bundle --tag contracts-latest

# Solution 2: Verify specific release
./scripts/smoke-verify-release.sh contracts-latest

# Solution 3: Temporarily skip verification (not recommended)
export DEMON_SKIP_BUNDLE_VERIFICATION=1
systemctl restart demon-runtime

# Monitor verification after restart
curl localhost:3000/api/contracts/status | jq '.contractBundle.alerts'
```

### Stale Bundle Alerts

**Symptoms:**
- `bundle.stale_detected` audit events
- Warning: "Bundle may be stale"
- Bundle age exceeds threshold

**Diagnosis:**
```bash
# Check bundle age
curl localhost:3000/api/contracts/status | jq '.contractBundle'

# Check for newer releases
demonctl contracts list-releases --limit 5

# View current bundle timestamp
jq '.timestamp' .demon/contracts/manifest.json
```

**Resolution:**
```bash
# Update to latest release
demonctl contracts fetch-bundle --tag contracts-latest

# Or adjust staleness threshold if appropriate
export DEMON_CONTRACTS_STALE_THRESHOLD_HOURS=72
systemctl restart demon-runtime

# Verify bundle status after update
curl localhost:3000/api/contracts/status | jq '.contractBundle.status'
```

## Monitoring and Alerting

### Automated Monitoring Setup

Pre-configured monitoring assets are available in the repository:

- **Prometheus Alerts**: `monitoring/prometheus/bundle-alerts.yml`
- **Grafana Dashboard**: `monitoring/grafana/contract-bundle-dashboard.json`
- **Validation Script**: `scripts/validate-release.sh`

### Deploying Monitoring

```bash
# Deploy Prometheus alert rules
kubectl apply -f monitoring/prometheus/bundle-alerts.yml

# Import Grafana dashboard
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $GRAFANA_API_KEY" \
  -d @monitoring/grafana/contract-bundle-dashboard.json \
  http://grafana.company.com/api/dashboards/db

# Set up automated validation checks (cron example)
echo "0 */6 * * * /path/to/demon/scripts/validate-release.sh >> /var/log/bundle-validation.log 2>&1" | crontab -
```

### Key Metrics to Monitor

**Bundle Status Metrics:**
```promql
# Bundle status (should be 1 for "loaded")
demon_bundle_status{state="loaded"}

# Bundle age in hours (alert if > 48)
demon_bundle_age_hours

# Bundle operation failures
rate(demon_bundle_operations_total{result="failed"}[5m])

# Verification failures (should be 0)
increase(demon_bundle_verification_failures_total[1h])
```

**Sample Alerting Rules:**
```yaml
groups:
  - name: bundle.rules
    rules:
    - alert: BundleVerificationFailed
      expr: increase(demon_bundle_verification_failures_total[1h]) > 0
      for: 0m
      annotations:
        summary: "Contract bundle verification failed"
        description: "Bundle SHA-256 verification has failed - check bundle integrity"

    - alert: BundleStale
      expr: demon_bundle_age_hours > 48
      for: 5m
      annotations:
        summary: "Contract bundle is stale"
        description: "Bundle is {{$value}} hours old - consider updating"

    - alert: BundleDownloadError
      expr: demon_bundle_status{state="download_error"} == 1
      for: 1m
      annotations:
        summary: "Failed to download contract bundle"
        description: "Check network connectivity and GitHub releases"
```

### Log Analysis

**Finding Bundle Events:**
```bash
# Filter for bundle audit events
journalctl -u demon-runtime | grep '"event":"bundle\.' | jq '.'

# Monitor real-time bundle events
journalctl -u demon-runtime -f | grep bundle

# Check for specific error patterns
journalctl -u demon-runtime | grep -E "(verification_failed|download_failed|fallback_activated)"
```

**Event Types to Monitor:**
- `bundle.loaded` - Successful bundle loading
- `bundle.verification_failed` - SHA-256 mismatch
- `bundle.download_failed` - Network/auth issues
- `bundle.fallback_activated` - Using embedded schemas
- `bundle.stale_detected` - Bundle age threshold exceeded

## Environment Configuration

### Production Settings

```bash
# Required settings
export DEMON_CONTRACTS_TAG=contracts-latest  # or specific tag
export DEMON_CONTRACTS_STALE_THRESHOLD_HOURS=24

# Optional settings
export DEMON_CONTRACTS_CACHE=/var/lib/demon/contracts
export GH_TOKEN=ghp_your_token_here  # for private repos

# Security settings (only for development)
# export DEMON_SKIP_BUNDLE_VERIFICATION=1  # DO NOT USE IN PRODUCTION
# export DEMON_SKIP_CONTRACT_BUNDLE=1      # Emergency only
```

### Network Configuration

**Corporate Proxies:**
```bash
export HTTPS_PROXY=http://proxy.company.com:8080
export HTTP_PROXY=http://proxy.company.com:8080

# For custom CA certificates
export SSL_CERT_FILE=/path/to/ca-bundle.crt

# Test connectivity
curl --proxy $HTTPS_PROXY -I https://github.com
```

**Firewall Requirements:**
- Outbound HTTPS (443) to github.com
- Outbound HTTPS (443) to api.github.com (for CLI operations)

## Scheduled Operations

### Daily Health Checks

```bash
#!/bin/bash
# daily-bundle-check.sh

set -euo pipefail

echo "=== Daily Bundle Health Check ==="

# Check bundle status
STATUS=$(curl -s localhost:3000/api/contracts/status | jq -r '.contractBundle.status')
echo "Bundle status: $STATUS"

if [ "$STATUS" != "loaded" ]; then
    echo "WARNING: Bundle not loaded properly"
    curl -s localhost:3000/api/contracts/status | jq '.contractBundle.alerts'
fi

# Check for new releases
echo "Latest releases:"
demonctl contracts list-releases --limit 3

# Verify current bundle
echo "Current bundle verification:"
cd .demon/contracts/
shasum -a 256 -c bundle.sha256 && echo "✓ Verification passed" || echo "✗ Verification failed"

echo "=== Check complete ==="
```

### Weekly Bundle Rotation

```bash
#!/bin/bash
# weekly-bundle-update.sh

set -euo pipefail

echo "=== Weekly Bundle Update ==="

# Backup current bundle
cp -r .demon/contracts/ .demon/contracts.backup.$(date +%Y%m%d)

# Update to latest
demonctl contracts fetch-bundle --tag contracts-latest

# Verify new bundle
shasum -a 256 -c .demon/contracts/bundle.sha256

# Restart runtime to load new bundle
systemctl restart demon-runtime

# Wait for startup
sleep 10

# Verify bundle loaded
STATUS=$(curl -s localhost:3000/api/contracts/status | jq -r '.contractBundle.status')
if [ "$STATUS" = "loaded" ]; then
    echo "✓ Bundle update successful"
    rm -rf .demon/contracts.backup.*
else
    echo "✗ Bundle update failed - check logs"
    exit 1
fi

echo "=== Update complete ==="
```

## Testing and Validation

### Automated Release Validation

The comprehensive validation script is available at `scripts/validate-release.sh`:

```bash
# Basic usage - validate latest release
./scripts/validate-release.sh

# Advanced usage examples
./scripts/validate-release.sh contracts-20250921-0658fb8b  # Specific tag
./scripts/validate-release.sh --verbose                    # Detailed output
DEMONCTL_BIN=./target/release/demonctl ./scripts/validate-release.sh  # Use compiled binary

# Integration with monitoring
./scripts/validate-release.sh && echo "✅ Release validation passed" || echo "❌ Release validation failed"
```

**Script features:**
- ✅ Comprehensive file and integrity verification
- ✅ Bundle structure validation via demonctl
- ✅ Metadata cross-validation
- ✅ Freshness checks with warnings
- ✅ Colored output and progress indicators
- ✅ Verbose mode for debugging
- ✅ Configurable demonctl binary path

### Load Testing

```bash
#!/bin/bash
# bundle-load-test.sh

# Test bundle loading performance
for i in {1..10}; do
    rm -rf .demon/contracts/
    time demonctl contracts fetch-bundle --tag contracts-latest > /dev/null
    echo "Iteration $i complete"
done

# Test concurrent access
for i in {1..5}; do
    (curl -s localhost:3000/api/contracts/status > /dev/null) &
done
wait

echo "Load test complete"
```

## Recovery Procedures

### Emergency Fallback

```bash
# Emergency: Disable bundle loading entirely
export DEMON_SKIP_CONTRACT_BUNDLE=1
systemctl restart demon-runtime

# Runtime will use embedded schemas without bundle dependencies
echo "Bundle loading disabled - using embedded schemas"
```

### Bundle Cache Recovery

```bash
# Clear corrupted cache
rm -rf .demon/contracts/

# Restore from known-good release
demonctl contracts fetch-bundle --tag contracts-20250920-4c99ca47

# Verify restoration
shasum -a 256 -c .demon/contracts/bundle.sha256
curl localhost:3000/api/contracts/status | jq '.contractBundle.status'
```

### Rollback to Previous Release

```bash
# List recent releases
demonctl contracts list-releases --limit 10

# Rollback to specific version
export DEMON_CONTRACTS_TAG=contracts-20250919-abcd1234
systemctl restart demon-runtime

# Monitor rollback success
curl localhost:3000/api/contracts/status | jq '.contractBundle'
```

## Contact and Escalation

**Primary Contacts:**
- Platform Team: platform-team@company.com
- On-call: oncall-platform@company.com

**Escalation Path:**
1. Check this playbook for resolution steps
2. Review logs and metrics for root cause
3. Contact platform team with diagnostic information
4. If critical: use emergency fallback procedures

**Required Information for Support:**
- Bundle status from `/api/contracts/status`
- Recent audit events with bundle.* prefix
- Current environment configuration
- Error messages from logs
- Steps already attempted

---

## Appendix

### Useful Commands Reference

```bash
# Bundle management
demonctl contracts list-releases --limit 10
demonctl contracts fetch-bundle --tag TAG --dest DIR
demonctl contracts validate FILE

# Status and monitoring
curl localhost:3000/api/contracts/status | jq '.'
curl localhost:3000/metrics | grep demon_bundle

# Debugging
export RUST_LOG=runtime::audit=info
journalctl -u demon-runtime -f | grep bundle

# Cache management
ls -la .demon/contracts/
shasum -a 256 .demon/contracts/bundle.json
```

### Environment Variables Reference

```bash
# Bundle configuration
DEMON_CONTRACTS_TAG=contracts-latest           # Release tag to use
DEMON_CONTRACTS_CACHE=.demon/contracts         # Cache directory
DEMON_CONTRACTS_STALE_THRESHOLD_HOURS=24       # Staleness threshold

# GitHub configuration
GH_TOKEN=ghp_xxx                               # GitHub token
GH_OWNER=afewell-hh                           # Repository owner
GH_REPO=demon                                 # Repository name

# Safety controls
DEMON_SKIP_CONTRACT_BUNDLE=1                  # Disable bundle loading
DEMON_SKIP_BUNDLE_VERIFICATION=1              # Skip SHA verification

# Logging
RUST_LOG=runtime::audit=info                  # Enable audit events
```

### Troubleshooting Checklist

- [ ] Check bundle status via API
- [ ] Verify GitHub connectivity
- [ ] Check for bundle corruption (SHA mismatch)
- [ ] Review audit events in logs
- [ ] Test manual bundle download
- [ ] Verify environment configuration
- [ ] Check disk space in cache directory
- [ ] Validate GitHub token permissions
- [ ] Test with known-good release tag
- [ ] Consider temporary emergency fallback