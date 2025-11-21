# Sprint D Demo — Canvas, Contracts Browser & Agent Flows

**Target Date**: 2025-11-21 14:00 PT
**Duration**: 30 minutes
**Audience**: Product stakeholders, engineering leads, potential users

---

## Agenda

1. **Context** (3 min) — Sprint D goals and v0.4.0 scope
2. **Canvas UI** (8 min) — Interactive DAG visualization demo
3. **Contracts Browser** (7 min) — Schema registry exploration
4. **Agent Flow API** (10 min) — Programmatic flow authoring
5. **Q&A** (2 min) — Questions and feedback

---

## Slide 1: Context — Sprint D Deliverables

### Problem Statement
Operations and developer teams need:
- **Visibility** into complex ritual execution flows
- **Discovery** of available contracts and capsule capabilities
- **Automation** for LLM agents to author and submit workflows

### Sprint D Solutions
- **Canvas UI** → Visualize ritual DAGs with live telemetry
- **Contracts Browser** → Explore schema registry and WIT definitions
- **Agent Flow API** → Programmatic flow authoring for AI agents
- **demonctl flow CLI** → Export/import workflow manifests

### Release Target
**v0.4.0** tagged 2025-11-21 after regression run

---

## Slide 2: Prerequisites & Setup

### Before Demo
```bash
# Start infrastructure
make dev  # NATS JetStream on 4222/8222

# Build workspace
cargo build --workspace

# Enable all Sprint D features
export OPERATE_UI_FLAGS=canvas-ui,contracts-browser,agent-flows
export SCHEMA_REGISTRY_URL=http://localhost:8080
export JWT_SECRET="demo-secret-key"

# Start Operate UI
cargo run -p operate-ui
```

### Verification
- http://localhost:3000/ → Operate UI home
- http://localhost:3030/canvas → Canvas UI loads
- http://localhost:3000/ui/contracts → Contracts Browser loads

---

## Slide 3: Canvas UI — Overview

### What is Canvas?
Interactive DAG (Directed Acyclic Graph) visualization of ritual execution flows with real-time telemetry overlays.

### Key Features
- **Force-directed graph layout** powered by D3.js v7
- **Node types**: Rituals, Capsules, Streams, Gates, UI Endpoints, Policies, Infrastructure
- **Telemetry overlays**: Color-coded lag/latency on edges (green < 50ms, amber 50-150ms, red > 150ms)
- **Interactive inspector**: Click nodes to view metadata, contracts, and status
- **Navigation**: Zoom/pan/reset, minimap for large graphs
- **Accessibility**: Keyboard navigation (Escape, Tab, Enter/Space)

### Architecture
- **Frontend**: D3.js + vanilla JavaScript, no framework dependencies
- **Backend**: Feature-flagged route in operate-ui (returns 404 when disabled)
- **Data source** (current): Embedded mock data for MVP demonstration
- **Data source** (future): Live integration with `demonctl inspect --graph` and NATS JetStream telemetry

---

## Slide 4: Canvas UI — Live Demo

### Demo Script

**Step 1: Navigate to Canvas**
```bash
open http://localhost:3030/canvas
```

**Expected**: Force-directed graph renders with 7 mock nodes (ritual, capsule, stream, gate, UI endpoint, policy, infrastructure)

**Step 2: Explore Node Types**
- **Point out colors**: Blue (ritual), Green (capsule), Orange (stream), Purple (gate), Cyan (UI endpoint), Red (policy), Blue-grey (infrastructure)
- **Hover over edges**: Show lag/latency telemetry badges

**Step 3: Inspect a Node**
- **Click the "echo@1.0.0" capsule node**
- **Expected**: Inspector panel slides in showing:
  - Node ID: `capsule-echo`
  - Type: `capsule`
  - Status: `completed`
  - Contract link → navigates to Contracts Browser
  - Metadata (version, last execution timestamp)

**Step 4: Navigate the Graph**
- **Zoom In** (+) → Magnify graph
- **Zoom Out** (−) → Shrink graph
- **Reset View** (⟳) → Return to default zoom/pan
- **Minimap** → Click bottom-right overview to jump to different graph regions

**Step 5: Keyboard Navigation**
- **Press Escape** → Close inspector
- **Tab** → Navigate controls
- **Enter/Space** → Activate buttons

**Step 6: Connection Status**
- **Point out connection indicator** (top-right)
- **Explain**: Shows "Connected" / "Reconnecting" / "Offline" states
- **Note**: Mock demo simulates offline toggle every 30 seconds

---

## Slide 5: Canvas UI — Future Roadmap

### Planned Enhancements (Post-MVP)

**Live Telemetry Integration**
- Replace mock data with real NATS JetStream stream (`SCALE_HINTS`)
- Server-Sent Events (SSE) endpoint: `/api/canvas/telemetry/stream`
- Real-time updates as ritual executes

**Run-Specific Views**
- Filter DAG by `?run_id=<run_id>` parameter
- Show only nodes/edges for specific ritual execution

**Tenant Filtering**
- Multi-tenant support with `?tenant=<tenant>` parameter
- Tenant selector dropdown in UI

**Historical Playback**
- Timeline scrubber to replay past ritual executions
- "Diff view" to compare two runs side-by-side

**Custom Layouts**
- Switch between force-directed, hierarchical, and radial layouts
- Save/load custom node positions

**Performance Optimizations**
- Virtualization for graphs > 100 nodes
- Canvas/WebGL rendering for 500+ nodes
- Static layout caching for repeated views

---

## Slide 6: Contracts Browser — Overview

### What is Contracts Browser?
Web-based interface for exploring the schema registry, browsing contracts, and viewing JSON schemas + WIT definitions.

### Key Features
- **Contract discovery**: Browse all available contracts from schema registry
- **Real-time search**: Filter by contract name, version, or author
- **Detail view**: Full contract metadata, JSON schema, and WIT definitions
- **Schema preview**: First 40 lines with expand/collapse toggle
- **Download**: Export contract schemas as JSON files
- **Accessibility**: Keyboard navigation, ARIA roles

### Configuration
- **Feature flag**: `contracts-browser`
- **Registry URL**: `SCHEMA_REGISTRY_URL` (default: `http://localhost:8080`)

---

## Slide 7: Contracts Browser — Live Demo

### Demo Script

**Step 1: Navigate to Contracts Browser**
```bash
open http://localhost:3000/ui/contracts
```

**Expected**: List of available contracts from schema registry

**Step 2: Browse Contracts**
- **Show contract list** with columns: Name, Version, Author, Description
- **Point out**: Each contract shows capsule capabilities and schemas

**Step 3: Search and Filter**
- **Type "echo" in search box**
- **Expected**: Filter list to show only echo-related contracts
- **Clear search** → Full list returns

**Step 4: View Contract Details**
- **Click "View" button on echo contract**
- **Expected**: Drawer slides in showing:
  - Contract name + version
  - Author and description
  - JSON Schema preview (first 40 lines)
  - "Expand Full Schema" toggle
  - WIT definition (if available)

**Step 5: Download Schema**
- **Click "Download Schema" button**
- **Expected**: Browser downloads `echo-v1-schema.json`

**Step 6: Close Drawer**
- **Press Escape** or click close button
- **Expected**: Drawer slides out, returns to contract list

**Step 7: Feature Flag Gating**
```bash
# Disable feature flag
unset OPERATE_UI_FLAGS

# Restart UI
cargo run -p operate-ui

# Navigate to /ui/contracts
open http://localhost:3000/ui/contracts
```
**Expected**: 404 Not Found (feature disabled)

---

## Slide 8: Agent Flow API — Overview

### What is Agent Flow API?
REST and NATS APIs for LLM agents and automated tools to programmatically author and submit flow manifests.

### Key Features
- **JWT authentication**: Scoped tokens (`flows:read`, `flows:write`)
- **Contract discovery**: `GET /api/contracts` lists available capsules/schemas
- **Flow drafting**: `POST /api/flows/draft` saves flows without validation (future)
- **Flow submission**: `POST /api/flows/submit` validates and registers flows
- **API versioning**: Header-based negotiation (`X-Demon-API-Version: v1`)
- **Idempotency**: Optional `Idempotency-Key` header for safe retries
- **Rate limiting**: 10 requests/minute per caller (configurable)

### Flow Manifest Schema
- **Schema version**: `v1` (current stable)
- **Metadata**: `flow_id`, `name`, `created_by`, `tags`
- **Nodes**: Array of workflow steps (trigger, task, capsule, approval, completion)
- **Edges**: Transitions between nodes with optional conditions
- **Bindings** (optional): Output-to-input mappings
- **Provenance** (optional): Agent ID, generation timestamp, source, parent flow

---

## Slide 9: Agent Flow API — Live Demo

### Demo Script

**Step 1: Generate JWT Token (for demo)**
```bash
# Set JWT secret
export JWT_SECRET="demo-secret-key"

# Create a simple test token (for demo only, use proper JWT library in production)
# Payload: {"sub": "demo-agent", "scope": "flows:read flows:write"}
export JWT_TOKEN="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJkZW1vLWFnZW50Iiwic2NvcGUiOiJmbG93czpyZWFkIGZsb3dzOndyaXRlIn0.SIGNATURE"
```

**Step 2: List Available Contracts**
```bash
curl -H "Authorization: Bearer $JWT_TOKEN" \
     -H "X-Demon-API-Version: v1" \
     http://localhost:3000/api/contracts
```

**Expected**: JSON array of contracts:
```json
[
  {
    "name": "echo",
    "kind": "capsule",
    "version": "v1",
    "description": "Echo capsule contract"
  }
]
```

**Step 3: Submit Agent-Authored Flow**
```bash
curl -X POST \
     -H "Authorization: Bearer $JWT_TOKEN" \
     -H "Content-Type: application/json" \
     -H "X-Demon-API-Version: v1" \
     -d @examples/flows/hello-agent.json \
     http://localhost:3000/api/flows/submit
```

**Expected**: Success response:
```json
{
  "flow_id": "hello-agent-001",
  "manifest_digest": "sha256:abc123...",
  "validation_result": {
    "valid": true,
    "errors": [],
    "warnings": []
  },
  "submitted_at": "2025-11-08T12:34:56Z"
}
```

**Step 4: Validation Failure Example**
```bash
# Submit invalid manifest (missing required field)
curl -X POST \
     -H "Authorization: Bearer $JWT_TOKEN" \
     -H "Content-Type: application/json" \
     -H "X-Demon-API-Version: v1" \
     -d '{"schema_version": "v1", "metadata": {}, "nodes": [], "edges": []}' \
     http://localhost:3000/api/flows/submit
```

**Expected**: Validation error response:
```json
{
  "flow_id": "",
  "manifest_digest": "sha256:...",
  "validation_result": {
    "valid": false,
    "errors": [
      {
        "code": "flow.metadata.flow_id_missing",
        "message": "flow_id field is required",
        "path": "metadata.flow_id"
      }
    ],
    "warnings": []
  }
}
```

**Step 5: Authentication Failure**
```bash
# Submit without JWT token
curl -X POST \
     -H "Content-Type: application/json" \
     -H "X-Demon-API-Version: v1" \
     -d @examples/flows/hello-agent.json \
     http://localhost:3000/api/flows/submit
```

**Expected**: 401 Unauthorized:
```json
{
  "error": "missing_token",
  "message": "Authorization header not provided"
}
```

---

## Slide 10: demonctl Flow CLI — Overview

### What is `demonctl flow`?
CLI commands for exporting ritual definitions as flow manifests and importing/submitting agent-authored flows.

### Commands
- **`demonctl flow export`** → Convert ritual YAML to flow manifest (JSON or YAML)
- **`demonctl flow import`** → Validate and submit flow manifest to API

### Use Cases
- **Export existing rituals** for editing/customization
- **Validate flow manifests** locally before API submission
- **Integrate with CI/CD** pipelines for automated flow deployment
- **Agent tooling** for LLM-driven workflow generation

---

## Slide 11: demonctl Flow CLI — Live Demo

### Demo Script

**Step 1: Export Ritual as Flow Manifest**
```bash
cargo run -p demonctl -- flow export \
  --ritual echo \
  --output /tmp/echo-flow.json
```

**Expected Output:**
```
✓ Exported flow manifest to: /tmp/echo-flow.json
  Flow ID: flow-echo
  Nodes: 3
  Edges: 2
```

**Step 2: Inspect Generated Manifest**
```bash
cat /tmp/echo-flow.json | jq .
```

**Expected**: Complete flow manifest with:
- `schema_version: "v1"`
- `metadata`: `flow-echo`, "Echo Ritual", `created_by: "demonctl-cli"`
- `nodes`: start (trigger), state_0 (task), complete (completion)
- `edges`: start → state_0 → complete
- `provenance`: agent_id, generation_timestamp, source

**Step 3: Validate Flow Manifest (Dry-Run)**
```bash
cargo run -p demonctl -- flow import \
  --file /tmp/echo-flow.json \
  --dry-run
```

**Expected Output:**
```
✓ Manifest validation passed
  Flow ID: flow-echo
  Name: Echo Ritual
  Nodes: 3
  Edges: 2

  Dry-run mode: not submitting to API
```

**Step 4: Submit Flow to API**
```bash
export DEMONCTL_JWT="$JWT_TOKEN"

cargo run -p demonctl -- flow import \
  --file /tmp/echo-flow.json \
  --api-url http://localhost:3000
```

**Expected Output:**
```
✓ Flow submitted successfully
  Flow ID: flow-echo
  Digest: sha256:def456...
  Submitted at: 2025-11-08T14:22:35Z
```

**Step 5: Export to YAML**
```bash
cargo run -p demonctl -- flow export \
  --ritual echo \
  --output /tmp/echo-flow.yaml
```

**Expected**: YAML-formatted flow manifest (same structure as JSON)

---

## Slide 12: Integration Example — LLM Agent Workflow

### Scenario
An LLM agent (e.g., Claude, GPT-4) generates a custom workflow and submits it to Demon.

### Workflow

**Step 1: Agent discovers available capsules**
```bash
curl -H "Authorization: Bearer $AGENT_JWT" \
     http://localhost:3000/api/contracts
```

**Step 2: Agent generates flow manifest**
```python
import json

flow = {
    "schema_version": "v1",
    "metadata": {
        "flow_id": "agent-deploy-001",
        "name": "Automated Deployment Flow",
        "created_by": "claude-agent",
        "tags": ["deployment", "agent-generated"]
    },
    "nodes": [
        {"node_id": "start", "type": "trigger", "config": {"trigger_type": "manual"}},
        {"node_id": "validate", "type": "capsule", "config": {"capsule_name": "validate-config", "inputs": {...}}},
        {"node_id": "deploy", "type": "capsule", "config": {"capsule_name": "deploy-app", "inputs": {...}}},
        {"node_id": "approval", "type": "approval", "config": {"gate_id": "ops-approval", "approvers": ["ops@company.com"]}},
        {"node_id": "complete", "type": "completion", "config": {"status": "success"}}
    ],
    "edges": [
        {"from": "start", "to": "validate"},
        {"from": "validate", "to": "deploy"},
        {"from": "deploy", "to": "approval"},
        {"from": "approval", "to": "complete", "condition": "approved"}
    ],
    "provenance": {
        "agent_id": "claude-agent-v1",
        "generation_timestamp": "2025-11-08T14:30:00Z",
        "source": "llm-generated"
    }
}

with open("/tmp/agent-flow.json", "w") as f:
    json.dump(flow, f, indent=2)
```

**Step 3: Agent validates locally**
```bash
demonctl flow import --file /tmp/agent-flow.json --dry-run
```

**Step 4: Agent submits to API**
```bash
demonctl flow import --file /tmp/agent-flow.json --api-url http://localhost:3000
```

**Step 5: Agent monitors execution**
```bash
# Future: Poll /api/flows/<flow_id>/status
# Future: Subscribe to NATS JetStream for flow execution events
```

---

## Slide 13: Success Metrics & Validation

### Sprint D Acceptance Criteria

**Canvas UI**
- [x] Feature-flagged route returns 404 when disabled
- [x] Force-directed graph renders with D3.js v7
- [x] Node inspector opens/closes on click/Escape
- [x] Zoom/pan/reset controls functional
- [x] Minimap displays and updates viewport
- [x] Keyboard navigation (Tab, Enter/Space, Escape)
- [x] Accessibility audit passes (axe-core, no violations)

**Contracts Browser**
- [x] Feature-flagged route returns 404 when disabled
- [x] Contract list loads from schema registry
- [x] Search/filter by name, version, author
- [x] Detail drawer shows full schema and WIT
- [x] Download schema as JSON
- [x] Keyboard navigation (Escape to close)

**Agent Flow API**
- [x] JWT authentication with scopes (`flows:read`, `flows:write`)
- [x] `GET /api/contracts` returns contract list
- [x] `POST /api/flows/submit` validates and accepts flows
- [x] Validation errors returned with structured error codes
- [x] API versioning header (`X-Demon-API-Version: v1`)
- [x] Idempotency support via `Idempotency-Key` header

**demonctl flow CLI**
- [x] `flow export` converts ritual to JSON/YAML manifest
- [x] `flow import --dry-run` validates manifest locally
- [x] `flow import` submits to API with JWT authentication
- [x] Error messages include actionable troubleshooting steps

### Quality Gates
- [x] `make fmt && make lint && make test` pass
- [x] Playwright E2E tests cover all UI features
- [x] Contract validation passes (`contracts-validate`)
- [x] Bootstrapper bundle verification (offline + negative)
- [x] Review-lock updated on every push
- [x] All review comments replied and resolved

---

## Slide 14: Documentation & Enablement

### New Documentation

**Comprehensive Guides**
- **`docs/canvas-ui.md`** (593 lines) — Architecture, configuration, API integration, troubleshooting
- **`docs/agent-flows.md`** (635 lines) — CLI workflow, manifest schema, examples, integration
- **`docs/agent-api.md`** (269 lines) — REST endpoints, JWT auth, error codes, security

**README Updates**
- **"Visualizing & Authoring Flows"** section added
- Quick-start examples for Canvas UI, Contracts Browser, Agent Flow API
- Links to comprehensive documentation

**AGENTS.md Updates**
- **"Visualization & Agent Flow Quick-Refs (Sprint D)"** section
- Command snippets for enabling features and using CLI

**docs/operate-ui/README.md Updates**
- **"Canvas UI"** section with features, configuration, troubleshooting
- Cross-links to Canvas UI documentation

### Enablement Assets
- Example flow manifests (`examples/flows/hello-agent.json`)
- Demo deck (this document)
- Integration examples (Python agent workflow)

---

## Slide 15: Known Limitations & Future Work

### Current Limitations

**Canvas UI**
- Mock data only (no live integration with `demonctl inspect`)
- No tenant/run filtering
- No historical playback or diff view
- Performance untested for graphs > 100 nodes

**Contracts Browser**
- Requires external schema registry (not bundled)
- No inline WIT editing or validation
- No contract versioning/diff view

**Agent Flow API**
- No `/api/flows/draft` endpoint (save without validation)
- No flow execution control (`/api/flows/<id>/start`, `/api/flows/<id>/stop`)
- No flow status polling (`/api/flows/<id>/status`)
- JWT issuer must be manually configured (no built-in Auth0 integration)

### Post-v0.4.0 Roadmap

**Q1 2026**
- Live telemetry integration (NATS JetStream → Canvas UI via SSE)
- Tenant/run filtering for Canvas and Contracts Browser
- Flow execution control API (`start`, `stop`, `status`)

**Q2 2026**
- Historical playback and diff view in Canvas
- Inline WIT editing in Contracts Browser
- Built-in JWT issuer integration (Auth0, Okta)

**Q3 2026**
- Agent SDK for Python, JavaScript, Go
- Flow template library (pre-built workflows)
- Multi-agent collaboration (shared flow authoring)

---

## Slide 16: Q&A

### Questions?

**Common Questions**

**Q: Can agents trigger flow execution after submission?**
A: Not yet. v0.4.0 supports flow **submission** only. Execution control (`/api/flows/<id>/start`) is planned for Q1 2026.

**Q: How does JWT authentication work in production?**
A: Configure `JWT_SECRET`, `JWT_ISSUER`, and `JWT_AUDIENCE` environment variables. Integrate with Auth0, Okta, or any OIDC-compliant provider.

**Q: What happens if an agent submits an invalid flow?**
A: API returns 400 Bad Request with structured error codes (e.g., `flow.metadata.flow_id_missing`) and actionable messages.

**Q: Can I visualize flows before submitting them?**
A: Yes! Use `demonctl flow export` → edit manifest → load in Canvas UI (future: direct manifest upload to Canvas).

**Q: How do I report bugs or request features?**
A: Open an issue at https://github.com/afewell-hh/Demon/issues with label `area:frontend` or `area:backend`.

---

## Slide 17: Next Steps

### For Stakeholders
- **Review this demo deck** and provide feedback via GitHub issues
- **Test Sprint D features** in dev environment (feature flags enabled)
- **Plan v0.5.0 roadmap** based on user feedback and telemetry data

### For Engineering
- **Complete regression testing** by 2025-11-21 11:00 PT
- **Tag v0.4.0 release** after all checks green
- **Update docs/mvp/01-mvp-contract.md** with Sprint D checkbox progress
- **Prepare v0.5.0 backlog** (live telemetry, flow execution control, SDK)

### For Users
- **Enable feature flags** and explore Canvas UI + Contracts Browser
- **Try demonctl flow CLI** to export/import workflows
- **Experiment with Agent Flow API** using JWT tokens
- **Provide feedback** on GitHub issues or Slack

---

## Appendix: Demo Checklist

### Pre-Demo Setup (15 min before)
- [ ] `make dev` (NATS running on 4222/8222)
- [ ] `cargo build --workspace` (all binaries ready)
- [ ] `export OPERATE_UI_FLAGS=canvas-ui,contracts-browser,agent-flows`
- [ ] `export SCHEMA_REGISTRY_URL=http://localhost:8080`
- [ ] `export JWT_SECRET="demo-secret-key"`
- [ ] `cargo run -p operate-ui` (UI server running)
- [ ] Verify http://localhost:3000/ (home page loads)
- [ ] Verify http://localhost:3030/canvas (Canvas UI renders)
- [ ] Verify http://localhost:3000/ui/contracts (Contracts Browser loads)
- [ ] Prepare JWT token: `export JWT_TOKEN="..."`
- [ ] Export echo ritual: `demonctl flow export --ritual echo --output /tmp/echo-flow.json`
- [ ] Test API: `curl -H "Authorization: Bearer $JWT_TOKEN" http://localhost:3000/api/contracts`

### During Demo
- [ ] Canvas UI: Navigate, inspect node, zoom/pan, keyboard navigation
- [ ] Contracts Browser: Search, view details, download schema
- [ ] Agent Flow API: List contracts, submit flow, validation errors, auth failure
- [ ] demonctl flow CLI: Export, validate (dry-run), submit

### Post-Demo
- [ ] Answer Q&A
- [ ] Collect feedback (GitHub issues, Slack)
- [ ] Share demo recording link
- [ ] Update epic status in docs/mvp/02-epics.md

---

## Contact & Resources

**Documentation**
- Canvas UI: docs/canvas-ui.md
- Agent Flows CLI: docs/agent-flows.md
- Agent Flow API: docs/agent-api.md
- Operate UI: docs/operate-ui/README.md

**Source Code**
- GitHub: https://github.com/afewell-hh/Demon
- Sprint D Epic: Issue #324

**Feedback**
- GitHub Issues: https://github.com/afewell-hh/Demon/issues
- Slack: #demon-team

---

## Demo Recording

**Sprint D v0.4.0 Demo** (Target: 2025-11-21 14:00 PT)

### Recording Details
- **Status**: Placeholder - Recording to be scheduled
- **Planned Duration**: ~30 minutes
- **Planned Topics**:
  - Canvas UI interactive DAG visualization (0:00-8:00)
  - Contracts Browser schema exploration (8:00-15:00)
  - Agent Flow API programmatic flow authoring (15:00-25:00)
  - demonctl flow CLI export/import commands (25:00-30:00)

### Environment Configuration
When the demo is recorded, it will use:
- **Feature Flags**: `OPERATE_UI_FLAGS=canvas-ui,contracts-browser,agent-flows`
- **Schema Registry**: `SCHEMA_REGISTRY_URL=http://localhost:8080`
- **JWT Secret**: `JWT_SECRET="demo-secret-key"` (demo only)
- **Infrastructure**: NATS JetStream on ports 4222/8222
- **Dataset**: Mock data for Canvas UI; live schema registry for Contracts Browser

### Access Instructions
Once the recording is published, assets will be available via:
- **Primary Storage**: GitHub Release v0.4.0 (MP4 attachment or external link)
- **Backup/Raw Assets**: Contact repository maintainers for access requests
- **Format**: MP4, 1080p minimum resolution
- **Verification**: SHA256 checksum will be provided with all artifacts

**Distribution Channels** (when available):
- This document (link updated in recording details section)
- Sprint D Epic (#324) final status comment
- Project README and CHANGELOG.md references

### Placeholder Checksum
```
# Checksum will be added after recording is captured
# Example format: sha256:abc123def456...
Recording: [PENDING]
Checksum:  [PENDING]
```

---

**End of Demo Deck**
**Sprint D — Canvas, Contracts Browser & Agent Flows**
**v0.4.0 Release Candidate**
