# Changelog

## v0.4.0 - Sprint D: Schema Visualization & Agent Empowerment (2025-11-09)

### New Features

**Operate UI - Contracts Browser** (Story S1, PR #331)
- Web-based interface for exploring the schema registry
- Real-time contract search and filtering by name, version, or author
- Detailed contract view with JSON schemas and WIT definitions
- Schema download functionality for offline use
- Feature flag gated (`contracts-browser`)
- Keyboard accessible interface with ARIA roles

**Operate UI - Canvas DAG Viewer** (Story S2, PR #333)
- Interactive force-directed graph visualization of ritual execution flows
- Real-time telemetry overlays showing lag/latency metrics on edges
- Node inspector panel with metadata and contract links
- Zoom/pan/reset controls with minimap for navigation
- Multiple node types: Rituals, Capsules, Streams, Gates, UI Endpoints, Policies, Infrastructure
- Color-coded telemetry thresholds (green <50ms, amber 50-150ms, red >150ms)
- Feature flag gated (`canvas-ui`)
- Keyboard navigation support (Escape, Tab, Enter/Space)
- Accessibility audit passing (axe-core)

**Agent Flow REST & NATS API** (Story S3, PR #334)
- JWT-authenticated REST API for programmatic flow authoring
- Contract discovery endpoint (`GET /api/contracts`)
- Flow submission endpoint (`POST /api/flows/submit`) with validation
- API versioning via `X-Demon-API-Version` header
- Idempotency support with `Idempotency-Key` header
- Rate limiting (10 requests/minute configurable)
- Flow manifest schema v1 with metadata, nodes, edges, bindings, and provenance
- Structured error responses with actionable error codes

**demonctl Flow CLI** (Story S4, PR #335)
- `demonctl flow export` - Convert ritual YAML to flow manifest (JSON/YAML)
- `demonctl flow import` - Validate and submit flow manifests to API
- `--dry-run` mode for local validation without submission
- JWT authentication support via `DEMONCTL_JWT` environment variable
- Colored, formatted output with progress indicators
- Integration with Agent Flow API

**Documentation & Enablement Pack** (Story S5, PR #336)
- Comprehensive Canvas UI documentation (`docs/canvas-ui.md`, 593 lines)
- Agent Flows CLI guide (`docs/agent-flows.md`, 635 lines)
- Agent Flow API reference (`docs/agent-api.md`, 269 lines)
- Updated README with "Visualizing & Authoring Flows" section
- Updated AGENTS.md with "Visualization & Agent Flow Quick-Refs" section
- Sprint D demo deck (`docs/preview/beta/canvas_agent_demo.md`, 731 lines)
- Example flow manifests in `examples/flows/`

**UI Snapshots & CI Guardrails** (Story S6, PR #338)
- Playwright visual regression testing for Canvas UI
- Playwright visual regression testing for Contracts Browser
- Snapshot verification in CI pipeline
- Automated screenshot comparison for UI changes
- Test artifacts captured and linked in CI runs

### Enhancements

- operate-ui: Feature flag system for progressive feature rollout
- operate-ui: JWT authentication middleware for API endpoints
- operate-ui: Improved error handling with structured error responses
- demonctl: Enhanced CLI UX with colored output and progress indicators
- docs: Expanded troubleshooting guides for Canvas UI and Contracts Browser

### CI/CD Improvements

- CI: Playwright UI test suite integrated into GitHub Actions
- CI: UI snapshot verification as required check
- CI: Feature flag testing in build pipeline

### Sprint D Deliverables Summary

- 6 stories completed (S1-S6): Contracts Browser, Canvas UI, Agent Flow API, CLI tools, documentation, UI snapshots
- 47/52 Playwright E2E tests passing (90% pass rate)
- All workspace Rust tests passing
- API versioning and authentication infrastructure
- Feature flag system for controlled rollout
- Comprehensive documentation for developers and operators

### Known Limitations

- Canvas UI uses embedded mock data (live telemetry integration planned for v0.5.0)
- No tenant/run filtering in Canvas UI
- No flow execution control endpoints (`/api/flows/<id>/start`, `/api/flows/<id>/stop`)
- Agent Flow API requires manual JWT configuration

### See Also

- Epic #324: Sprint D — Schema Visualization & Agent Empowerment
- Milestone: Sprint D — Schema Visualization & Agent Empowerment

---

## v0.3.0 - Prior Release

## Unreleased (pre-v0.3.0)

- docs: CI for bundle provenance — positive & negative offline verify; key rotation + determinism guidance
- demonctl: enforce Cosign signature verification (hashed key, bundle validation) and drop `--allow-unsigned`
- demonctl: add ritual alias support (`demonctl run <app>:<ritual>`) backed by installed App Packs
