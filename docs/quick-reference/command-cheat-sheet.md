# Command Cheat Sheet

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Quick reference for common Demon workflows and commands.

## Development Setup

```bash
# Build the workspace
make build

# Start dev environment
make dev

# Run tests
make test

# Format and lint
make fmt && make lint

# Quick smoke test
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

## Docker & Containers

```bash
# Build runtime image
docker build -f runtime/Dockerfile -t demon-runtime .

# Run with compose
make up    # Start NATS and dependencies
make down  # Stop services

# Check container logs
docker logs demon-runtime
docker logs demon-engine
```

## Documentation

```bash
# Check documentation links
./scripts/check-doc-links.sh

# Check external links too
./scripts/check-doc-links.sh --external

# Check specific directory
./scripts/check-doc-links.sh docs/tutorials

# Quiet mode for CI
./scripts/check-doc-links.sh --quiet
```

## API Testing

```bash
# Check API health
curl http://localhost:3000/api/runs

# List active runs
curl http://localhost:3000/api/runs | jq

# Get run details
curl http://localhost:3000/api/runs/{run_id} | jq

# Grant approval
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/grant \
  -H "Content-Type: application/json"

# Deny approval
curl -X POST http://localhost:3000/api/approvals/{run_id}/{gate_id}/deny \
  -H "Content-Type: application/json" \
  -d '{"reason": "Policy violation"}'
```

## Git & CI

```bash
# Update review lock (replace PR_NUM and SHA)
gh pr edit PR_NUM --body "$(gh pr view PR_NUM -q .body)\n\nReview-lock: SHA"

# Check PR status
gh pr checks PR_NUM

# Run branch protection checks locally
./scripts/check-branch-protection.sh  # (if available)

# View CI logs
gh run view RUN_ID --log
```

## Troubleshooting

```bash
# Check service status
kubectl get pods -n demon-system

# View pod logs
kubectl logs -n demon-system deployment/demon-engine
kubectl logs -n demon-system deployment/demon-runtime

# Check NATS connection
nats-cli stream ls
nats-cli consumer ls RITUAL_EVENTS

# Debug networking
kubectl port-forward -n demon-system svc/demon-engine 3000:3000
```

## Quick Diagnostics

| Issue | Command | Expected Result |
|-------|---------|----------------|
| **Build failing** | `make build` | No errors, binaries created |
| **Tests failing** | `make test` | All tests pass |
| **API not responding** | `curl localhost:3000/api/runs` | JSON response |
| **NATS issues** | `nats-cli stream ls` | Shows RITUAL_EVENTS |
| **Pod crashes** | `kubectl get pods` | All pods Running |

## Environment Variables

```bash
# Core configuration
export NATS_URL="nats://localhost:4222"
export RITUAL_STREAM_NAME="RITUAL_EVENTS"
export LOG_LEVEL="info"

# Development overrides
export RUST_LOG="debug"
export RUST_BACKTRACE="1"
```

## See Also

- [Tutorials](../tutorials/) - Step-by-step learning guides
- [API Reference](../api/) - Complete API documentation
- [Operations](../ops/) - Production runbooks
- [Troubleshooting](./troubleshooting-decision-tree.md) - Interactive problem solver

[← Back to Documentation Home](../README.md)