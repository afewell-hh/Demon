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

# Graph capsule tests
cargo test -p capsules_graph

# Runtime graph dispatch tests
cargo test -p runtime --test graph_dispatch_spec
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

# Graph capsule: Check JetStream events
nats stream info GRAPH_COMMITS
nats stream view GRAPH_COMMITS

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

## Graph Commands

```bash
# Create graph with seed mutations
demonctl graph create \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  mutations.json

# Commit mutations
demonctl graph commit \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  --parent-ref <COMMIT_ID> \
  mutations.json

# Tag a commit
demonctl graph tag \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1 \
  --tag v1.0.0 \
  --commit-id <COMMIT_ID>

# List tags
demonctl graph list-tags \
  --tenant-id tenant-1 \
  --project-id proj-1 \
  --namespace ns-1 \
  --graph-id graph-1

# Get commit by ID (REST)
curl "http://localhost:8080/api/graph/commits/<COMMIT_ID>?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# List commits (REST)
curl "http://localhost:8080/api/graph/commits?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1&limit=50"

# Get tag (REST)
curl "http://localhost:8080/api/graph/tags/v1.0.0?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# List all tags (REST)
curl "http://localhost:8080/api/graph/tags?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"

# Query operations (submit as mutations via demonctl graph commit)
# - get-node: retrieve node by ID with labels/properties/edges
# - neighbors: find connected nodes (filtered by relType/direction)
# - path-exists: check if path exists between two nodes

# View graphs in Operate UI
open http://localhost:3000/graph
# Or with specific scope:
open "http://localhost:3000/graph?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"
```

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