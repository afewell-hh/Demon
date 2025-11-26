# End-to-End Validation for Sprint D Features

This document defines the canonical user journeys for Sprint D features and describes the automated validation suite that ensures these journeys work correctly before any human demo or recording.

## Purpose

Sprint D introduces critical operator and agent-facing features:
- **Contracts Browser** (Story S1): Schema registry explorer for operators
- **Canvas UI** (Story S2): Interactive DAG visualization for ritual flows
- **Agent Flow API** (Story S3): Programmatic flow authoring for LLM agents
- **demonctl flow CLI** (Story S4): Export/import workflows via command line

This validation suite codifies the canonical user workflows, executes them automatically in CI, and provides confidence that the integrated system works end-to-end before customer demos or recordings.

## Governance & Process Alignment

This E2E validation strategy aligns with the project's governance standards defined in:
- **AGENTS.md**: Review-lock discipline, replies policy, TDD expectations
- **README.md**: Required checks, quickstart flows, API versioning
- **docs/process/ui_snapshot_workflow.md**: UI snapshot testing and feature flag patterns
- **docs/process/PM_REBOOT_PLAYBOOK.md**: CI invariants, token usage, thin slices

### Test-Driven Development (TDD) Expectations

Per AGENTS.md:24-28, all new features must have accompanying tests. This E2E suite extends the existing test coverage to validate:
1. **UI flows**: Playwright tests with deterministic data seeding
2. **CLI/API flows**: Rust integration tests with local runtime instances
3. **Feature flag behavior**: Tests verify features are gated correctly

### CI Integration Requirements

Per AGENTS.md:70-76, required checks on `main` must not be renamed. The E2E validation suite runs as:
- **Non-required job** initially (advisory only, does not block merges)
- **Runs on PRs** that touch `operate-ui/`, `demonctl/`, or `runtime/`
- **Artifacts available** for debugging (HTML reports, logs)
- **Future promotion** to required check when stabilized

## Canonical User Journeys

### Journey 1: Operator Inspects Contracts via Contracts Browser

**Persona**: Platform operator validating schema compatibility before deploying a new capsule version.

**Given**: Operate UI is running with `contracts-browser` feature flag enabled, and a schema registry is accessible.

**When**: The operator navigates to the Contracts Browser UI and searches for a specific contract.

**Then**:
- The operator can browse all available contracts in the registry
- The operator can search/filter contracts by name, version, or author
- The operator can view full contract metadata, JSON schemas, and WIT definitions
- The operator can download contract schemas for offline validation
- The operator can compare schema versions to identify breaking changes

**Acceptance**:
- UI loads at `/ui/contracts` with 200 response
- Search functionality filters contracts in real-time
- Contract detail drawer displays schema and metadata
- Keyboard navigation (Escape, Tab, Enter) works as expected
- Empty state and error states display gracefully
- No visual regressions (snapshot tests pass)

### Journey 2: Operator Visualizes Ritual Flows via Canvas UI

**Persona**: Operations engineer investigating ritual execution paths and telemetry during an incident.

**Given**: Operate UI is running with `canvas-ui` feature flag enabled, and telemetry data is available.

**When**: The operator navigates to the Canvas viewer and interacts with the DAG.

**Then**:
- The operator sees an interactive force-directed graph of ritual nodes
- The operator can zoom, pan, and reset the view
- The operator can inspect individual nodes (rituals, capsules, streams, gates, policies)
- The operator can view live telemetry overlays (lag/latency on edges)
- The operator can navigate from Canvas back to Contracts Browser via node inspector links
- Minimap provides spatial awareness for large graphs

**Acceptance**:
- Canvas loads at `/canvas` with 200 response
- SVG rendering completes without errors
- Control buttons (zoom, pan, reset) function correctly
- Minimap reflects current viewport position
- Node inspector shows contract links
- Keyboard accessibility (Escape closes inspector, Tab navigates)
- No visual regressions (snapshot tests pass)

### Journey 3: Agent Authors and Submits Flow Manifest via CLI/API

**Persona**: LLM agent drafting a new workflow based on available contracts and submitting it for execution.

**Given**: Agent has access to the Demon API, contract registry, and `demonctl` CLI.

**When**: The agent exports an example ritual, modifies it, validates the manifest, and submits it via API.

**Then**:
- Agent can export `examples/rituals/echo.yaml` to a flow manifest (JSON/YAML)
- Agent can validate the manifest schema using `demonctl flow import --dry-run`
- Agent can query `/api/contracts` to discover available contracts
- Agent can POST the manifest to `/api/flows/submit` with valid JWT
- Agent can verify the submitted flow appears in Operate UI at `/runs` or `/api/flows`

**Acceptance**:
- `demonctl flow export --ritual echo --output flow.json` produces valid JSON manifest
- Manifest adheres to `contracts/schemas/flow_manifest.v1.json` schema
- `demonctl flow import --file flow.json --dry-run` validates successfully
- `demonctl flow import --file flow.json --api-url http://localhost:3000` submits flow (with JWT)
- Submitted flow is queryable via `/api/flows` and appears in Operate UI
- Integration test exercises full workflow programmatically

## Test Implementation Strategy

### UI Flows: Playwright Tests

**Location**: `operate-ui/playwright/tests/e2e_contracts_browser.spec.ts`, `operate-ui/playwright/tests/e2e_canvas_ui.spec.ts`

**Seeding Strategy**:
- Deterministic mock data loaded via scripted fixture (no remote dependencies)
- Registry mocks provide consistent contract list
- Telemetry mocks provide stable DAG for Canvas rendering
- Seeding script: `examples/seed/seed_e2e_ui.sh` (new)

**Test Structure** (Given/When/Then):
```typescript
test("Operator searches and inspects contracts end-to-end", async ({ page, baseURL }) => {
  // Given: Contracts Browser is loaded with seeded data
  await page.goto(`${baseURL}/ui/contracts`);
  await page.waitForSelector("#search-input");

  // When: Operator searches for a specific contract
  await page.fill("#search-input", "ritual.started");
  await page.waitForTimeout(500); // Debounce

  // Then: Search results show filtered contracts
  const contractCards = page.locator(".contract-card");
  await expect(contractCards).toHaveCount(1);

  // When: Operator clicks on a contract
  await contractCards.first().click();

  // Then: Detail drawer opens with schema
  const drawer = page.locator("#detail-drawer");
  await expect(drawer).toHaveClass(/open/);
  await expect(drawer).toContainText("ritual.started:v1");

  // When: Operator downloads schema
  const downloadButton = page.locator("#download-schema");
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    downloadButton.click(),
  ]);
  expect(download.suggestedFilename()).toContain("ritual.started");

  // When: Operator closes drawer with Escape
  await page.keyboard.press("Escape");

  // Then: Drawer closes
  await expect(drawer).not.toHaveClass(/open/);
});
```

**Snapshot Testing**:
- Visual snapshots captured for each major UI state
- Deterministic viewport (1280x720)
- Animations disabled for consistency
- Threshold: 0.2, maxDiffPixels: 100 (per `ui_snapshot_workflow.md`)

### CLI/API Flows: Rust Integration Tests

**Location**: `demonctl/tests/flow_e2e_integration_spec.rs` (new)

**Test Structure** (Given/When/Then):
```rust
#[tokio::test]
#[ignore] // Requires NATS and runtime
async fn agent_exports_validates_and_submits_flow_end_to_end() -> Result<()> {
    // Given: Local runtime is started with agent-flows feature flag
    let runtime = start_test_runtime("agent-flows").await?;
    let api_url = "http://127.0.0.1:3000";

    // When: Agent exports echo ritual
    let output = temp_path("echo_flow.json");
    export_ritual("echo", &output)?;

    // Then: Export produces valid manifest
    let manifest: FlowManifest = serde_json::from_str(&fs::read_to_string(&output)?)?;
    assert_eq!(manifest.schema_version, "v1");
    assert_eq!(manifest.metadata.flow_id, "flow-echo-ritual");

    // When: Agent validates manifest
    let validate_result = validate_manifest(&output)?;

    // Then: Validation passes
    assert!(validate_result.is_valid);

    // When: Agent queries available contracts
    let contracts = query_contracts(&api_url, &runtime.jwt).await?;

    // Then: Contracts are returned
    assert!(!contracts.is_empty());
    assert!(contracts.iter().any(|c| c.name.contains("ritual.started")));

    // When: Agent submits flow manifest
    let submission = submit_flow(&api_url, &output, &runtime.jwt).await?;

    // Then: Submission succeeds
    assert_eq!(submission.status, "accepted");
    assert_eq!(submission.flow_id, "flow-echo-ritual");

    // When: Agent queries submitted flows
    let flows = query_flows(&api_url, &runtime.jwt).await?;

    // Then: Submitted flow appears
    assert!(flows.iter().any(|f| f.flow_id == "flow-echo-ritual"));

    runtime.shutdown().await?;
    Ok(())
}
```

**Helper Functions**:
- `start_test_runtime(flags: &str) -> TestRuntime`: Starts Operate UI with feature flags and JWT
- `export_ritual(name: &str, output: &Path) -> Result<()>`: Calls `demonctl flow export`
- `validate_manifest(file: &Path) -> Result<ValidationResult>`: Calls `demonctl flow import --dry-run`
- `submit_flow(api_url: &str, file: &Path, jwt: &str) -> Result<Submission>`: POSTs to `/api/flows/submit`
- `query_contracts(api_url: &str, jwt: &str) -> Result<Vec<Contract>>`: GETs `/api/contracts`
- `query_flows(api_url: &str, jwt: &str) -> Result<Vec<Flow>>`: GETs `/api/flows`

### Deterministic Test Data

**Registry Mocks**: `examples/seed/contracts_mock.json`
- Fixed set of contracts: `ritual.started:v1`, `ritual.completed:v1`, `capsule.executed:v1`
- Predictable metadata (author, created_at, version)

**Telemetry Mocks**: `examples/seed/telemetry_mock.json`
- Stable DAG with 5 nodes, 4 edges
- Fixed lag/latency values for reproducibility

**Seeding Scripts**:
- `examples/seed/seed_e2e_ui.sh`: Seeds registry and telemetry mocks for Playwright tests
- Invoked before Playwright runs in CI and locally via helper script

## CI Integration

### New CI Job: `e2e-validation`

**Location**: `.github/workflows/ci.yml` (DO NOT RENAME existing jobs per AGENTS.md:70-76)

**Trigger**: PRs that modify `operate-ui/`, `demonctl/`, `runtime/`, or Playwright tests

**Status**: Non-required (advisory only, does not block merges)

**Steps**:
1. Start NATS (JetStream)
2. Build workspace
3. Seed E2E test data (`examples/seed/seed_e2e_ui.sh`)
4. Start Operate UI with feature flags (`OPERATE_UI_FLAGS=contracts-browser,canvas-ui,agent-flows`)
5. Wait for HTTP 200 from `/api/runs` (per AGENTS.md:86)
6. Run Playwright E2E tests (`npx playwright test --grep "end-to-end"`)
7. Run Rust integration tests (`cargo test -p demonctl flow_e2e_integration_spec -- --ignored --nocapture`)
8. Upload artifacts (HTML reports, logs, screenshots) on failure
9. Teardown NATS

**Artifacts**:
- `playwright-report/`: HTML report with screenshots and traces
- `e2e-logs/`: Operate UI logs, demonctl output
- `manifests/`: Exported flow manifests for inspection

**Retries**: 2 retries in CI for flake tolerance (per `ui_snapshot_workflow.md`)

### Future Promotion to Required Check

When the E2E suite is stabilized and flake-free:
1. Add `e2e-validation` to branch protection required checks (coordinate with protection snapshot)
2. Document in `.github/snapshots/branch-protection-YYYY-MM-DD.json`
3. Update `AGENTS.md` and `README.md` to reflect new required check

## Local Validation Workflow

### Prerequisites

```bash
# Start NATS
make up

# Seed E2E test data
./examples/seed/seed_e2e_ui.sh

# Start Operate UI with all feature flags
export OPERATE_UI_FLAGS=contracts-browser,canvas-ui,agent-flows
export JWT_SECRET=test-secret-local
cargo run -p operate-ui
```

### Run E2E Tests Locally

```bash
# Playwright UI tests
cd operate-ui/playwright
npm install
npx playwright test --grep "end-to-end"

# View report
npx playwright show-report

# Rust integration tests
cargo test -p demonctl flow_e2e_integration_spec -- --ignored --nocapture
```

### Update Snapshots

If UI changes require snapshot updates:
```bash
./scripts/update-ui-snapshots.sh
git diff operate-ui/tests/__artifacts__/snapshots/
```

Per `ui_snapshot_workflow.md`, commit updated snapshots with UI changes and document in PR description.

## Debugging E2E Test Failures

### Playwright Failures

1. **Check artifacts**: CI uploads HTML report, screenshots, and traces
2. **Reproduce locally**: Run with `--debug` flag for headed mode
3. **Verify seeding**: Ensure `seed_e2e_ui.sh` ran successfully
4. **Check feature flags**: Ensure `OPERATE_UI_FLAGS` includes all required flags
5. **Review logs**: Operate UI logs in `e2e-logs/operate-ui.log`

### Rust Integration Test Failures

1. **Check NATS**: Ensure JetStream is running and streams are created
2. **JWT validation**: Verify `JWT_SECRET` matches between runtime and test
3. **API availability**: Confirm Operate UI is accessible at `http://127.0.0.1:3000`
4. **Manifest validation**: Inspect exported manifest JSON in `manifests/`
5. **Contract schema**: Ensure contract schemas are up-to-date in `contracts/schemas/`

## Maintenance & Evolution

### When to Update This Suite

- **New Sprint D features**: Add new journeys and tests
- **Contract schema changes**: Update mocks and validation assertions
- **API versioning**: Add tests for new API versions (per `api-versioning.md`)
- **Feature flag changes**: Update seeding scripts and test setup
- **UI redesigns**: Regenerate snapshots and update locators

### Flake Mitigation

Per `ui_snapshot_workflow.md:175-201`, common flake causes:
- **Animations**: Disable via `animations: "disabled"` in snapshots
- **Loading states**: Wait for specific elements, not arbitrary timeouts
- **Dynamic data**: Use deterministic mocks, no timestamps/random IDs
- **Platform differences**: Increase threshold if cross-platform rendering varies

### Coordination with PR Workflow

Per AGENTS.md:82-89, E2E validation integrates with PR lifecycle:
1. **Before PR**: Run E2E tests locally, ensure passing
2. **In PR**: CI runs E2E suite, uploads artifacts
3. **Review**: Reviewers check E2E artifacts if tests fail
4. **Merge**: E2E passing (or non-blocking if advisory)
5. **Post-merge**: Monitor for regressions in subsequent PRs

## Evidence & Reporting

### For Demo Preparation

Per issue #343, customer demos must rely solely on automated validation:
1. **Run full E2E suite**: `make e2e-validate` (new Makefile target)
2. **Review artifacts**: Ensure all journeys pass
3. **Verify snapshots**: No visual regressions
4. **Check logs**: No errors or warnings in Operate UI logs
5. **Document**: Link CI run URL in demo preparation doc (`docs/preview/beta/canvas_agent_demo.md`)

### For Issue #343 Closure

When E2E suite is complete:
1. **Comment on issue #343** with:
   - Links to PRs implementing the suite
   - CI run URLs showing passing E2E tests
   - Sample artifacts (HTML reports, logs, exported manifests)
   - Summary of journeys covered and tests added
2. **Update `docs/preview/beta/canvas_agent_demo.md`**: Note reliance on automated validation
3. **Update `CHANGELOG.md`**: Note E2E validation suite in "Known Limitations" or "Testing"

## See Also

- [AGENTS.md](../../AGENTS.md) — Review-lock, replies policy, required checks
- [README.md](../../README.md) — Quickstart, API versioning, feature flags
- [ui_snapshot_workflow.md](ui_snapshot_workflow.md) — UI snapshot testing and feature flag patterns
- [PM_REBOOT_PLAYBOOK.md](PM_REBOOT_PLAYBOOK.md) — CI invariants, token usage
- [docs/canvas-ui.md](../canvas-ui.md) — Canvas UI architecture and configuration
- [docs/operate-ui/README.md](../operate-ui/README.md) — Contracts Browser guide
- [docs/agent-flows.md](../agent-flows.md) — Flow export/import CLI
- [docs/agent-api.md](../agent-api.md) — Agent Flow API authentication and endpoints
