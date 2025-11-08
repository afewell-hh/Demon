# Canvas UI — Interactive DAG Visualization

The Canvas UI provides an interactive, real-time visualization of ritual execution flows with embedded telemetry overlays. It renders execution graphs as force-directed DAGs, showing capsules, state streams, approval gates, UI endpoints, and policy wards with live lag/latency metrics on edges.

## Quick Start

```bash
# Enable the Canvas UI feature
export OPERATE_UI_FLAGS=canvas-ui

# Start the Operate UI
cargo run --bin operate-ui

# Navigate to Canvas UI
open http://localhost:3030/canvas
```

## Prerequisites

1. **Feature Flag** — Set `OPERATE_UI_FLAGS=canvas-ui` to enable the Canvas viewer
2. **NATS JetStream** — Running instance for ritual event streaming (optional for mock data)
3. **Modern Browser** — Support for SVG, ES6 modules, and D3.js v7
4. **Telemetry Stream** — Scale hints or edge metrics for live telemetry overlays (future)

## Feature Overview

### Core Capabilities

- **DAG Rendering** — Force-directed graph layout with D3.js showing ritual execution flows
- **Node Types** — Visual distinction for rituals, capsules, streams, gates, UI endpoints, policies, and infrastructure
- **Telemetry Overlays** — Live lag/latency metrics on edges with color-coded thresholds
- **Node Inspector** — Detailed metadata panel with contract links and execution status
- **Zoom/Pan/Reset** — Interactive navigation with mouse wheel zoom, drag-to-pan, and reset button
- **Minimap** — Overview panel with viewport indicator for large graphs
- **Offline Handling** — Connection status indicator with automatic reconnection attempts
- **Keyboard Accessibility** — Escape to close inspector, Tab navigation, Enter/Space activation

### Node Type Color Coding

| Node Type     | Color      | Description                              |
|---------------|------------|------------------------------------------|
| Ritual        | Blue       | Top-level workflow orchestrator          |
| Capsule       | Green      | WebAssembly execution unit               |
| Stream        | Orange     | NATS JetStream event stream              |
| Gate          | Purple     | Approval gate requiring human decision   |
| UI Endpoint   | Cyan       | Operate UI exposure point                |
| Policy        | Red        | Policy ward for validation/guardrails    |
| Infrastructure| Blue-Grey  | Supporting infrastructure (NATS, etc.)   |

### Telemetry Thresholds

Edge telemetry uses color coding to indicate performance:

| Threshold      | Color | Latency Range    | Interpretation           |
|----------------|-------|------------------|--------------------------|
| Healthy        | Green | < 50ms           | Normal operation         |
| Warning        | Amber | 50ms - 150ms     | Elevated latency         |
| Critical       | Red   | > 150ms          | Performance degradation  |

## Architecture

### Frontend Stack

- **D3.js v7** — Force-directed graph layout, zoom/pan, SVG rendering
- **Vanilla JavaScript** — No framework dependencies for simplicity
- **Tera Templates** — Server-side rendering with feature flag gating
- **CSS Grid/Flexbox** — Responsive layout for controls and panels

### Data Flow (Current Implementation)

```
Canvas Route Handler (routes.rs)
    ↓
Feature Flag Check (canvas-ui)
    ↓
Tera Template Render (canvas_viewer.html)
    ↓
D3.js Force Simulation (client-side)
    ↓
Mock Data (embedded in template)
```

### Data Flow (Future with Live Telemetry)

```
demonctl inspect --graph --json --tenant <tenant>
    ↓
NATS JetStream (SCALE_HINTS stream)
    ↓
Server-Sent Events (SSE) /api/canvas/telemetry/stream
    ↓
Canvas UI (live updates via EventSource)
    ↓
D3.js Re-renders with Telemetry Overlays
```

## Feature Flag Configuration

The Canvas UI is gated behind the `canvas-ui` feature flag:

```bash
# Enable Canvas UI only
export OPERATE_UI_FLAGS=canvas-ui

# Enable multiple features (comma-separated)
export OPERATE_UI_FLAGS=canvas-ui,contracts-browser

# Disable all features (default)
unset OPERATE_UI_FLAGS
```

**Implementation:**
- Checked in `canvas_viewer_html()` route handler (`operate-ui/src/routes.rs:2499`)
- Returns `404 Not Found` when feature flag is disabled
- Navigation link hidden in base template when disabled (`operate-ui/templates/base.html:265-267`)

## User Interface

### Main Canvas Area

The central SVG canvas renders the DAG with:
- **Nodes** — Circles sized by importance, colored by type
- **Edges** — Curved paths with directional arrows
- **Telemetry Badges** — Lag/latency labels on edges
- **Zoom/Pan Controls** — Buttons in top-right corner
- **Minimap** — Bottom-right viewport overview

### Node Inspector Panel

Clicking a node opens a slide-out panel showing:
- **Node ID** — Unique identifier
- **Type** — Node classification
- **Status** — Current execution state
- **Contract Link** — Link to `/ui/contracts` for schema details
- **Metadata** — Additional properties (varies by node type)

**Keyboard Navigation:**
- `Escape` — Close inspector
- `Tab` / `Shift+Tab` — Navigate between interactive elements

### Controls Toolbar

Located at top of canvas:
- **Zoom In** (+) — Increase magnification
- **Zoom Out** (−) — Decrease magnification
- **Reset View** (⟳) — Return to default zoom/pan
- **Pause/Resume** (⏸/▶) — Freeze/unfreeze force simulation
- **Connection Status** — Indicator showing "Connected", "Reconnecting", or "Offline"

### Minimap

Bottom-right overview panel:
- **Viewport Rectangle** — Shows current visible area
- **Full Graph** — Miniaturized view of entire DAG
- **Click to Navigate** — Click minimap to jump to that region

## API Integration (Future)

### Telemetry Stream Endpoint

**Endpoint:** `GET /api/canvas/telemetry/stream`

**Response:** Server-Sent Events (SSE) stream

**Event Format:**
```json
{
  "event": "telemetry",
  "data": {
    "edges": [
      {
        "source": "ritual-abc123",
        "target": "capsule-echo",
        "lag": 5,
        "latency_ms": 42.5
      },
      {
        "source": "capsule-echo",
        "target": "stream-events",
        "lag": 120,
        "latency_ms": 180.2
      }
    ],
    "timestamp": "2025-01-06T15:30:45Z"
  }
}
```

### Graph Data Endpoint

**Endpoint:** `GET /api/canvas/graph?tenant=<tenant>&run_id=<run_id>`

**Response:**
```json
{
  "nodes": [
    {
      "id": "ritual-abc123",
      "type": "ritual",
      "label": "Deploy Application",
      "status": "running"
    },
    {
      "id": "capsule-echo",
      "type": "capsule",
      "label": "echo@1.0.0",
      "status": "completed"
    }
  ],
  "edges": [
    {
      "source": "ritual-abc123",
      "target": "capsule-echo",
      "type": "invoke"
    }
  ]
}
```

### Integration with `demonctl inspect`

The Canvas UI will consume data from the `demonctl inspect` command:

```bash
# Export graph data for Canvas UI
demonctl inspect --graph --json --tenant production \
  > /tmp/canvas-graph-data.json

# Canvas UI polls or streams this endpoint
# Backend transforms inspect output into Canvas format
```

See [demonctl inspect documentation](cli-inspect.md) for telemetry schema details.

## Mock Data (Current Implementation)

The initial implementation includes embedded mock data representing a typical ritual execution:

**Mock DAG Structure:**
```
Ritual (entry point)
  ├─→ Capsule (echo@1.0.0)
  │    └─→ Event Stream (demon.ritual.v1.events)
  │         └─→ NATS JetStream (infrastructure)
  ├─→ Approval Gate (deploy-gate)
  │    └─→ UI Endpoint (/api/approvals/grant)
  │         └─→ Event Stream (subscription)
  └─→ Policy Ward (security-policy)
```

**Mock Telemetry:**
- Simulated lag values: 0-150 messages
- Simulated latency: 20ms - 200ms
- Updates every 1 second with randomization
- Connection simulation (toggles offline/reconnecting every 30 seconds)

## Testing Strategy

### Unit Tests

**Location:** `operate-ui/src/routes.rs`

**Test Coverage:**
- Feature flag gating (returns 404 when disabled)
- Template rendering (returns 200 with HTML when enabled)
- Context variables (canvas_enabled, contracts_browser_enabled)
- Error handling (template render failures)

**Example Test:**
```rust
#[tokio::test]
async fn canvas_viewer_returns_404_when_feature_disabled() {
    // Ensure OPERATE_UI_FLAGS does not include canvas-ui
    std::env::remove_var("OPERATE_UI_FLAGS");

    let state = AppState::new().await;
    let result = canvas_viewer_html(State(state)).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.status_code, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn canvas_viewer_returns_html_when_feature_enabled() {
    std::env::set_var("OPERATE_UI_FLAGS", "canvas-ui");

    let state = AppState::new().await;
    let result = canvas_viewer_html(State(state)).await;

    assert!(result.is_ok());
    let html = result.unwrap();
    assert!(html.0.contains("<svg"));
    assert!(html.0.contains("Canvas DAG Viewer"));
}
```

### Playwright E2E Tests

**Location:** `operate-ui/tests/e2e/canvas.spec.ts`

**Test Scenarios:**
1. **Feature Flag Gating** — Verify 404 when feature disabled
2. **Page Load** — Canvas page renders without errors
3. **Graph Rendering** — DAG nodes and edges appear
4. **Node Interaction** — Click node opens inspector
5. **Inspector Close** — Escape key closes inspector
6. **Zoom Controls** — +/− buttons adjust zoom level
7. **Reset View** — Reset button returns to default view
8. **Minimap** — Minimap reflects viewport position
9. **Keyboard Navigation** — Tab cycles through interactive elements
10. **Accessibility Audit** — axe-core finds no violations

**Example Test:**
```typescript
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test.describe('Canvas UI', () => {
  test.beforeEach(async ({ page }) => {
    // Set feature flag via env or mock API
    await page.goto('http://localhost:3030/canvas');
  });

  test('renders DAG with nodes and edges', async ({ page }) => {
    await expect(page.locator('svg')).toBeVisible();
    await expect(page.locator('circle')).toHaveCount(7); // 7 mock nodes
    await expect(page.locator('path.edge')).toHaveCount(7); // 7 mock edges
  });

  test('opens node inspector on click', async ({ page }) => {
    await page.locator('circle[data-node-id="ritual-1"]').click();
    await expect(page.locator('#inspector')).toBeVisible();
    await expect(page.locator('#inspector')).toContainText('Ritual');
  });

  test('closes inspector with Escape key', async ({ page }) => {
    await page.locator('circle').first().click();
    await page.keyboard.press('Escape');
    await expect(page.locator('#inspector')).toBeHidden();
  });

  test('accessibility audit passes', async ({ page }) => {
    const accessibilityScanResults = await new AxeBuilder({ page })
      .analyze();
    expect(accessibilityScanResults.violations).toEqual([]);
  });
});
```

### Telemetry Fixtures

**Location:** `operate-ui/fixtures/canvas/`

**Files:**
- `canvas_dag_simple.json` — Minimal 3-node graph
- `canvas_dag_complex.json` — 20-node graph with multiple branches
- `canvas_telemetry_healthy.json` — All edges green
- `canvas_telemetry_degraded.json` — Mixed yellow/red edges
- `canvas_telemetry_critical.json` — All edges red

**Usage:**
```rust
// In tests, load fixture data
let fixture = include_str!("../fixtures/canvas/canvas_dag_simple.json");
let graph: GraphData = serde_json::from_str(fixture)?;
```

## Troubleshooting

### Canvas Page Returns 404

**Symptom:** Navigating to `/canvas` shows "404 - Page Not Found"

**Resolution:**
```bash
# Check feature flag is set
echo $OPERATE_UI_FLAGS

# Enable Canvas UI
export OPERATE_UI_FLAGS=canvas-ui

# Restart Operate UI
cargo run --bin operate-ui
```

### Navigation Link Not Visible

**Symptom:** "Canvas" link missing from header

**Cause:** Feature flag not set or `canvas_enabled` context variable not passed to all templates

**Resolution:**
```bash
# Verify all route handlers include canvas_enabled in context
grep -n "canvas_enabled" operate-ui/src/routes.rs

# Ensure base template includes conditional link
grep -A 2 "canvas_enabled" operate-ui/templates/base.html
```

### Graph Not Rendering

**Symptom:** Blank canvas or JavaScript console errors

**Possible Causes:**
- D3.js library failed to load
- Mock data structure invalid
- SVG rendering error

**Resolution:**
```javascript
// Check browser console for errors
// Verify D3 is loaded:
console.log(d3.version); // Should print "7.9.0"

// Inspect mock data:
console.log(graphData);
```

### Telemetry Not Updating

**Symptom:** Lag/latency values frozen

**Current Behavior:** Mock telemetry updates every 1 second with simulated values

**Future Resolution:**
- Verify SSE endpoint is streaming: `curl -N http://localhost:3030/api/canvas/telemetry/stream`
- Check `EventSource` connection status
- Verify NATS JetStream is emitting scale hints

## Performance Considerations

### Large Graphs (100+ Nodes)

For graphs exceeding 100 nodes:
- **Force Simulation** — May cause high CPU usage; consider static layouts
- **Canvas Rendering** — Explore HTML5 Canvas or WebGL instead of SVG
- **Data Pagination** — Load subgraphs on demand or filter by run_id/tenant

**Optimization Tips:**
```javascript
// Reduce simulation iterations for faster initial render
simulation.alpha(1).alphaDecay(0.05);

// Disable force simulation after stabilization
simulation.on('end', () => {
  console.log('Simulation complete, disabling physics');
});

// Use canvas instead of SVG for 500+ nodes
// Consider switching to pixi.js or three.js
```

### Network Bandwidth (SSE Streaming)

**Current:** Mock telemetry (no network)

**Future Considerations:**
- SSE streams can consume bandwidth; throttle updates to 1Hz or less
- Compress large graph payloads with gzip
- Use WebSocket for bi-directional communication if needed
- Implement reconnection backoff to avoid thundering herd

## Browser Compatibility

### Supported Browsers

- **Chrome/Edge** — v90+ (recommended)
- **Firefox** — v88+
- **Safari** — v14+
- **Opera** — v76+

### Unsupported

- Internet Explorer (no ES6 module support)
- Mobile browsers (limited testing; zoom/pan may behave differently)

### Polyfills

Not currently required. If targeting older browsers, consider:
- `core-js` for ES6 features
- `whatwg-fetch` for Fetch API
- `eventsource-polyfill` for SSE

## Security Considerations

### XSS Prevention

- All node labels and metadata are escaped in template rendering
- User-provided data (future) should be sanitized server-side
- CSP headers recommended for production deployments

### Authentication

- Canvas UI inherits authentication from Operate UI (currently no auth required)
- Future: Enforce ADMIN_TOKEN for telemetry streams
- Future: Tenant isolation for multi-tenant deployments

## Future Enhancements

### Planned Features (Post-MVP)

1. **Live Telemetry Integration** — Replace mock data with real NATS JetStream data
2. **Run-Specific Views** — Filter DAG by run_id parameter
3. **Tenant Filtering** — Multi-tenant support with tenant selector
4. **Historical Playback** — Replay ritual executions with timeline scrubber
5. **Custom Layouts** — Switch between force-directed, hierarchical, and radial layouts
6. **Export/Share** — Download graph as PNG/SVG or share via link
7. **Diff View** — Compare two ritual executions side-by-side
8. **Alert Overlays** — Highlight nodes/edges exceeding thresholds
9. **Search/Filter** — Filter nodes by type, status, or contract name
10. **Annotations** — Add user comments/notes to specific nodes

### Technical Debt

- Replace mock data with API integration
- Add Redux/Zustand for state management as complexity grows
- Extract D3 logic into separate modules for testability
- Implement virtualization for large graphs (react-window or similar)
- Add Storybook for component-level testing

## See Also

- [Operate UI Documentation](operate-ui/README.md) — Overview of all Operate UI features
- [demonctl inspect](cli-inspect.md) — CLI command for graph metrics inspection
- [Scale Feedback Telemetry](scale-feedback.md) — Runtime telemetry schema and configuration
- [Contracts Browser](operate-ui/README.md#contracts-browser) — Contract schema viewer (related feature)

## Contributing

When extending the Canvas UI:

1. **Follow TDD** — Write tests before implementation
2. **Update Fixtures** — Add new fixture files for new graph patterns
3. **Document Changes** — Update this file with new features/endpoints
4. **Accessibility First** — Ensure keyboard navigation and screen reader support
5. **Performance** — Test with 100+ node graphs before merging

## Appendix: D3.js Force Simulation Parameters

Current configuration for optimal graph layout:

```javascript
const simulation = d3.forceSimulation(nodes)
  .force('link', d3.forceLink(edges)
    .id(d => d.id)
    .distance(150))               // Edge length target
  .force('charge', d3.forceManyBody()
    .strength(-300))              // Node repulsion
  .force('center', d3.forceCenter(width / 2, height / 2))
  .force('collision', d3.forceCollide()
    .radius(30));                 // Prevent node overlap

// Adjust for different graph sizes:
// - Small graphs (< 10 nodes): distance=200, strength=-500
// - Medium graphs (10-50 nodes): distance=150, strength=-300
// - Large graphs (50+ nodes): distance=100, strength=-200
```

## Appendix: Telemetry Schema

Expected telemetry event format from NATS JetStream:

```json
{
  "schema_version": "1.0.0",
  "tenant": "default",
  "run_id": "abc123def456",
  "timestamp": "2025-01-06T15:30:45Z",
  "edges": [
    {
      "source_node_id": "ritual-1",
      "target_node_id": "capsule-echo",
      "lag": 5,
      "latency_ms": 42.5,
      "error_rate": 0.0
    }
  ]
}
```

**Field Descriptions:**
- `schema_version` — Telemetry format version (semver)
- `tenant` — Tenant identifier for multi-tenancy
- `run_id` — Unique ritual execution ID
- `timestamp` — ISO 8601 timestamp of measurement
- `edges[].source_node_id` — Source node in DAG
- `edges[].target_node_id` — Target node in DAG
- `edges[].lag` — Number of pending messages (queue depth)
- `edges[].latency_ms` — 95th percentile processing latency in milliseconds
- `edges[].error_rate` — Error rate as decimal (0.0 = 0%, 1.0 = 100%)
