# Agent Flow CLI — Export & Import Workflows

The `demonctl flow` commands enable developers and LLM agents to export ritual definitions as flow manifests and import/submit agent-authored flows to the Demon platform.

## Quick Start

```bash
# Export an existing ritual as a flow manifest
demonctl flow export --ritual echo --output my-flow.json

# Validate a flow manifest (dry-run)
demonctl flow import --file my-flow.json --dry-run

# Submit a flow manifest to the API
export DEMONCTL_JWT="your-jwt-token"
demonctl flow import --file my-flow.json --api-url http://localhost:3000
```

## Prerequisites

1. **demonctl CLI** — Built from workspace (`cargo build -p demonctl`)
2. **Agent Flow API** — Running instance with `agent-flows` feature flag enabled
3. **JWT Token** — Valid token with `flows:read` and `flows:write` scopes (for import/submit)
4. **Example Rituals** — Sample workflows in `examples/rituals/`

## Commands

### `demonctl flow export`

Export a ritual definition (YAML) as a flow manifest (JSON or YAML).

#### Syntax

```bash
demonctl flow export \
  --ritual <RITUAL_ID_OR_PATH> \
  --output <OUTPUT_FILE> \
  [--api-url <URL>]
```

#### Options

| Option | Short | Description | Environment Variable |
|--------|-------|-------------|---------------------|
| `--ritual` | `-r` | Ritual ID (from `examples/rituals/`) or path to YAML file | - |
| `--output` | `-o` | Output file path (`.json` or `.yaml` extension) | - |
| `--api-url` | - | Optional API URL for fetching ritual metadata | `DEMONCTL_API_URL` |

#### Examples

**Export by ritual ID:**
```bash
# Looks for examples/rituals/echo.yaml
demonctl flow export --ritual echo --output flows/echo-flow.json
```

**Export by file path:**
```bash
demonctl flow export \
  --ritual /path/to/my-ritual.yaml \
  --output my-flow.json
```

**Export as YAML:**
```bash
demonctl flow export \
  --ritual echo \
  --output flows/echo-flow.yaml
```

#### Output

The command generates a flow manifest conforming to `contracts/schemas/flow_manifest.v1.json`:

```json
{
  "schema_version": "v1",
  "metadata": {
    "flow_id": "flow-echo",
    "name": "Echo Ritual",
    "description": "Exported from echo.yaml",
    "created_by": "demonctl-cli",
    "tags": ["exported", "ritual-derived"]
  },
  "nodes": [
    {
      "node_id": "start",
      "type": "trigger",
      "config": {
        "trigger_type": "manual",
        "label": "Start Workflow"
      }
    },
    {
      "node_id": "state_0",
      "type": "task",
      "config": {
        "state_name": "EchoTask",
        "action": { "capsule": "echo", "inputs": {...} }
      }
    },
    {
      "node_id": "complete",
      "type": "completion",
      "config": {
        "status": "success",
        "message": "Flow completed successfully"
      }
    }
  ],
  "edges": [
    { "from": "start", "to": "state_0" },
    { "from": "state_0", "to": "complete" }
  ],
  "bindings": null,
  "provenance": {
    "agent_id": "demonctl",
    "generation_timestamp": "2025-11-08T12:34:56Z",
    "source": "cli-export",
    "parent_flow_id": null
  }
}
```

**Success Output:**
```
✓ Exported flow manifest to: flows/echo-flow.json
  Flow ID: flow-echo
  Nodes: 3
  Edges: 2
```

### `demonctl flow import`

Import and optionally submit a flow manifest to the Agent Flow API.

#### Syntax

```bash
demonctl flow import \
  --file <MANIFEST_FILE> \
  [--dry-run] \
  [--api-url <URL>] \
  [--jwt <TOKEN>]
```

#### Options

| Option | Short | Description | Environment Variable | Default |
|--------|-------|-------------|---------------------|---------|
| `--file` | `-f` | Path to flow manifest file (JSON or YAML) | - | - |
| `--dry-run` | - | Validate only, do not submit to API | - | `false` |
| `--api-url` | - | API URL for submission | `DEMONCTL_API_URL` | `http://localhost:3000` |
| `--jwt` | - | JWT token for authentication | `DEMONCTL_JWT` | - |

#### Examples

**Validate a manifest (dry-run):**
```bash
demonctl flow import \
  --file my-flow.json \
  --dry-run
```

**Submit to API:**
```bash
export DEMONCTL_JWT="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."

demonctl flow import \
  --file my-flow.json \
  --api-url http://localhost:3000
```

**Submit with inline JWT:**
```bash
demonctl flow import \
  --file my-flow.json \
  --jwt "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

**Import YAML manifest:**
```bash
demonctl flow import \
  --file my-flow.yaml \
  --api-url http://localhost:3000
```

#### Validation

The command performs local validation before submission:

- **Schema version** — Must be `"v1"`
- **Required fields** — `flow_id`, `name`, `created_by`, `nodes[]`
- **Unique node IDs** — No duplicate `node_id` values
- **Valid edges** — All `from` and `to` references must exist in `nodes[]`

**Validation Success (Dry-Run):**
```
✓ Manifest validation passed
  Flow ID: my-flow-001
  Name: My Agent Flow
  Nodes: 4
  Edges: 3

  Dry-run mode: not submitting to API
```

**Validation Failure:**
```
Error: Edge references non-existent node: nonexistent-node
```

#### API Submission

When `--dry-run` is not specified, the command submits the manifest to `POST /api/flows/submit` with:

**Request Headers:**
- `Authorization: Bearer <jwt-token>`
- `Content-Type: application/json`
- `X-Demon-API-Version: v1`

**Request Body:**
```json
{
  "manifest": { ... }
}
```

**Success Response:**
```
✓ Flow submitted successfully
  Flow ID: my-flow-001
  Digest: sha256:abc123...
  Submitted at: 2025-11-08T12:34:56Z
```

**Validation Errors:**
```
✗ Flow validation failed:
  - [flow.schema_version_missing] schema_version field is required
    Path: schema_version
  - [flow.nodes_missing] manifest.nodes must contain at least one node
    Path: nodes

Error: Flow manifest validation failed
```

**Warnings (Non-Blocking):**
```
✓ Flow submitted successfully
  Flow ID: my-flow-001
  Digest: sha256:abc123...
  Submitted at: 2025-11-08T12:34:56Z

  Warnings:
  - [flow.deprecated_field] Field 'bindings' is deprecated, use 'config.bindings' instead
```

#### Error Codes

| HTTP Status | Meaning | Resolution |
|-------------|---------|------------|
| `401 Unauthorized` | Missing or invalid JWT token | Check `DEMONCTL_JWT` environment variable |
| `403 Forbidden` | Insufficient scopes | Ensure token has `flows:write` scope |
| `400 Bad Request` | Invalid manifest structure | Review validation errors in response |
| `500 Internal Server Error` | API error | Check API logs and retry |

## Flow Manifest Schema

Flow manifests conform to JSON Schema at `contracts/schemas/flow_manifest.v1.json`.

### Core Structure

```json
{
  "schema_version": "v1",
  "metadata": { ... },
  "nodes": [ ... ],
  "edges": [ ... ],
  "bindings": { ... },       // optional
  "provenance": { ... }      // optional
}
```

### Metadata

```json
{
  "flow_id": "unique-flow-id",
  "name": "Human-readable name",
  "description": "Optional description",
  "created_by": "agent-id or user-id",
  "tags": ["tag1", "tag2"]   // optional
}
```

**Constraints:**
- `flow_id` must be unique within tenant namespace
- `name` must be non-empty
- `created_by` identifies the authoring agent or user

### Nodes

Nodes represent workflow steps (triggers, tasks, approvals, completions).

```json
{
  "node_id": "unique-node-id",
  "type": "trigger|task|capsule|approval|completion",
  "config": {
    // Type-specific configuration
  },
  "metadata": {
    "position": { "x": 100, "y": 200 }  // optional, for Canvas UI
  }
}
```

**Node Types:**

| Type | Description | Example Config |
|------|-------------|----------------|
| `trigger` | Workflow entry point | `{"trigger_type": "manual", "label": "Start"}` |
| `task` | Serverless Workflow task state | `{"state_name": "MyTask", "action": {...}}` |
| `capsule` | Direct capsule invocation | `{"capsule_name": "echo", "inputs": {...}}` |
| `approval` | Human approval gate | `{"gate_id": "gate-001", "approvers": [...]}` |
| `completion` | Workflow termination | `{"status": "success", "message": "Done"}` |

### Edges

Edges define transitions between nodes.

```json
{
  "from": "source-node-id",
  "to": "target-node-id",
  "condition": "approved|rejected|...",  // optional
  "metadata": { ... }                     // optional
}
```

**Constraints:**
- `from` and `to` must reference existing `node_id` values
- Conditional edges (e.g., approval outcomes) use `condition`

### Bindings (Optional)

Bindings map outputs from one node to inputs of another.

```json
{
  "binding-name": {
    "source": "source-node-id.output-field",
    "target": "target-node-id.input-field",
    "transform": "passthrough|jq-expression"
  }
}
```

### Provenance (Optional)

Tracks flow authorship and lineage.

```json
{
  "agent_id": "claude-agent-001",
  "generation_timestamp": "2025-11-08T12:34:56Z",
  "source": "api|cli-export|manual",
  "parent_flow_id": "parent-flow-123"  // if derived from another flow
}
```

## Example Workflow

### End-to-End: Export, Modify, Import

**Step 1: Export an existing ritual**
```bash
demonctl flow export \
  --ritual echo \
  --output base-flow.json
```

**Step 2: Edit the manifest**
```bash
# Edit base-flow.json in your editor
# Add an approval gate between task and completion nodes
cat base-flow.json | jq '.nodes += [{
  "node_id": "approval-gate",
  "type": "approval",
  "config": {
    "gate_id": "ops-approval",
    "approvers": ["ops@company.com"],
    "timeout_seconds": 3600
  }
}]' > enhanced-flow.json
```

**Step 3: Validate locally**
```bash
demonctl flow import \
  --file enhanced-flow.json \
  --dry-run
```

**Step 4: Submit to API**
```bash
export DEMONCTL_JWT="your-token-here"

demonctl flow import \
  --file enhanced-flow.json \
  --api-url http://localhost:3000
```

### Agent-Authored Flow

LLM agents can generate flows programmatically and submit via API or CLI:

**Python Example (using subprocess):**
```python
import subprocess
import json

# Generate flow manifest
flow = {
    "schema_version": "v1",
    "metadata": {
        "flow_id": "agent-generated-001",
        "name": "Agent-Created Workflow",
        "created_by": "python-agent",
        "tags": ["agent-generated"]
    },
    "nodes": [
        {"node_id": "start", "type": "trigger", "config": {"trigger_type": "manual"}},
        {"node_id": "task1", "type": "capsule", "config": {"capsule_name": "echo", "inputs": {"msg": "Hello"}}},
        {"node_id": "end", "type": "completion", "config": {"status": "success"}}
    ],
    "edges": [
        {"from": "start", "to": "task1"},
        {"from": "task1", "to": "end"}
    ]
}

# Write to file
with open("/tmp/agent-flow.json", "w") as f:
    json.dump(flow, f, indent=2)

# Submit via demonctl
result = subprocess.run([
    "demonctl", "flow", "import",
    "--file", "/tmp/agent-flow.json",
    "--api-url", "http://localhost:3000"
], capture_output=True, text=True, env={"DEMONCTL_JWT": "your-jwt-token"})

if result.returncode == 0:
    print("✓ Flow submitted:", result.stdout)
else:
    print("✗ Submission failed:", result.stderr)
```

## Integration with Agent Flow API

The `demonctl flow import` command integrates with endpoints documented in [Agent Flow API](agent-api.md):

- **List Contracts**: `GET /api/contracts` — Discover available capsules and schemas
- **Draft Flow**: `POST /api/flows/draft` — Save a flow without validation (future)
- **Submit Flow**: `POST /api/flows/submit` — Validate and register a flow for execution

See [docs/agent-api.md](agent-api.md) for JWT configuration, scopes, and error handling.

## Troubleshooting

### JWT Token Not Found

**Symptom:**
```
Error: JWT token required for API submission. Set --jwt flag or DEMONCTL_JWT environment variable
```

**Resolution:**
```bash
# Export JWT token
export DEMONCTL_JWT="your-token-here"

# Or pass inline
demonctl flow import --file my-flow.json --jwt "your-token-here"
```

### Validation Errors

**Symptom:**
```
Error: manifest.metadata.flow_id is required
```

**Resolution:**
Ensure your manifest includes all required fields:
```json
{
  "schema_version": "v1",
  "metadata": {
    "flow_id": "my-flow-001",        // required
    "name": "My Flow",               // required
    "created_by": "my-agent"         // required
  },
  "nodes": [ ... ],                  // required, must be non-empty
  "edges": [ ... ]
}
```

### API Connection Refused

**Symptom:**
```
Error: Failed to send request to API: connection refused
```

**Resolution:**
1. Verify API is running: `curl http://localhost:3000/api/contracts`
2. Check `--api-url` parameter or `DEMONCTL_API_URL` environment variable
3. Ensure `agent-flows` feature flag is enabled: `export OPERATE_UI_FLAGS=agent-flows`

### Unsupported Schema Version

**Symptom:**
```
Error: Unsupported schema version: v2. Expected 'v1'
```

**Resolution:**
Update manifest to use `"schema_version": "v1"`:
```bash
jq '.schema_version = "v1"' my-flow.json > my-flow-fixed.json
```

### Edge References Non-Existent Node

**Symptom:**
```
Error: Edge references non-existent node: task-99
```

**Resolution:**
Ensure all `from` and `to` values in `edges[]` match `node_id` values in `nodes[]`:
```bash
# List all node IDs
jq '.nodes[].node_id' my-flow.json

# Check edge references
jq '.edges[] | "\(.from) -> \(.to)"' my-flow.json
```

## Feature Flag

The Agent Flow API requires the `agent-flows` feature flag:

```bash
export OPERATE_UI_FLAGS=agent-flows
cargo run -p operate-ui
```

See [docs/agent-api.md#feature-flag](agent-api.md#feature-flag) for details.

## See Also

- [Agent Flow API](agent-api.md) — REST/NATS endpoints and authentication
- [Canvas UI](canvas-ui.md) — Visualize flows with interactive DAG viewer
- [Contracts Browser](operate-ui/README.md#contracts-browser) — Explore available capsule contracts
- [demonctl CLI Reference](../README.md#quickstart) — Overview of all CLI commands
- [Flow Manifest Schema](../contracts/schemas/flow_manifest.v1.json) — JSON Schema definition
