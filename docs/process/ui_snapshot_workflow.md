# UI Snapshot Workflow

This document describes the workflow for UI snapshot testing in the Demon project, including how to update snapshots, review changes, and troubleshoot issues.

## Overview

UI snapshot testing provides visual regression detection for the Operate UI, specifically for:
- **Contracts Browser** (`/ui/contracts`) - Schema registry explorer
- **Canvas UI** (`/canvas`) - Interactive DAG viewer

Snapshots are captured using Playwright and stored in `operate-ui/tests/__artifacts__/snapshots/`. When UI code changes, snapshot tests fail if the visual output differs from the baseline, alerting developers to unintended visual regressions.

## Architecture

### Snapshot Storage

Snapshots are stored at: `operate-ui/tests/__artifacts__/snapshots/{testFilePath}/{snapshotName}.png`

Example:
```
operate-ui/tests/__artifacts__/snapshots/
  canvas_ui.spec.ts/
    canvas-viewer.png
    canvas-with-controls.png
  contracts_browser.spec.ts/
    contracts-browser.png
    contracts-browser-header.png
```

### Test Configuration

Playwright configuration (`operate-ui/playwright/playwright.config.ts`):
- **Deterministic viewport**: 1280x720 pixels
- **Animations disabled**: Ensures consistent snapshots
- **Threshold**: 0.2 (allows minor cross-platform rendering differences)
- **Max diff pixels**: 100 (tolerates small pixel variations)

### CI Integration

The `ui-snapshots` job in `.github/workflows/ci.yml`:
- **Non-required**: Does not block merges (advisory only)
- **Runs on**: All PRs that modify `operate-ui/` or Playwright tests
- **Seeds data**: Calls `examples/seed/seed_preview.sh` before tests
- **Artifacts**: Uploads diff images on failure
- **Retries**: 2 retries in CI for flake tolerance

## Workflow

### When UI Code Changes

1. **Develop your UI feature** in `operate-ui/`
2. **Run tests locally** to see snapshot failures:
   ```bash
   cd operate-ui/playwright
   npm test
   ```
3. **Review failures**:
   - Playwright reports which snapshots differ
   - Check if visual changes are intentional
4. **Update snapshots** if changes are intentional:
   ```bash
   ./scripts/update-ui-snapshots.sh
   ```
5. **Review diffs**:
   ```bash
   git diff operate-ui/tests/__artifacts__/snapshots/
   ```
6. **Commit updated snapshots** with your UI changes

### Updating Snapshots

Use the helper script to regenerate all snapshots:

```bash
./scripts/update-ui-snapshots.sh
```

**Prerequisites:**
- NATS running: `make up`
- Operate UI running with feature flags:
  ```bash
  OPERATE_UI_FLAGS=contracts-browser,canvas-ui cargo run -p operate-ui
  ```
- (Optional) Seed data:
  ```bash
  ./examples/seed/seed_preview.sh
  ```

The script:
1. Checks prerequisites
2. Runs Playwright with `--update-snapshots`
3. Filters to only snapshot tests (via `--grep "visual snapshot"`)
4. Reports results

### Manual Snapshot Update

If you need fine-grained control:

```bash
cd operate-ui/playwright

# Update all snapshots
UPDATE_SNAPSHOTS=true npm test

# Update specific test file
UPDATE_SNAPSHOTS=true npx playwright test canvas_ui.spec.ts

# Update and view report
UPDATE_SNAPSHOTS=true npx playwright test --reporter=html
npx playwright show-report
```

### Reviewing Snapshot Changes

Before committing updated snapshots:

1. **Visual inspection**:
   ```bash
   # Use your image viewer to compare
   open operate-ui/tests/__artifacts__/snapshots/
   ```

2. **Git diff** (if snapshots are committed as text-representable):
   ```bash
   git diff --stat operate-ui/tests/__artifacts__/snapshots/
   ```

3. **Playwright report** (after test run):
   ```bash
   npx playwright show-report
   ```

4. **Validate intentionality**:
   - Are the visual changes expected from your code changes?
   - Do they match your design intent?
   - Are there unintended side effects?

## Review Expectations

### For PR Authors

When your PR modifies UI and updates snapshots:

1. **Document changes**: In PR description, explain:
   - What UI changes were made
   - Why snapshots were updated
   - Reference `scripts/update-ui-snapshots.sh` usage

2. **Attach evidence**:
   - Screenshots showing before/after (if helpful)
   - Link to CI run with snapshot test results

3. **Ensure determinism**:
   - Run snapshot update script multiple times
   - Verify snapshots are stable (no random elements)

### For Reviewers

When reviewing PRs with snapshot updates:

1. **Check intentionality**:
   - Do snapshot changes align with stated UI changes?
   - Are there unexpected visual regressions?

2. **Verify CI**:
   - Check `ui-snapshots` job status (even if non-required)
   - Review uploaded diff artifacts on failures

3. **Request clarification**:
   - If visual changes are unclear
   - If snapshots seem overly sensitive

## Troubleshooting

### Snapshot Tests Fail in CI But Pass Locally

**Cause**: Platform-specific rendering differences (fonts, anti-aliasing, etc.)

**Solutions**:
1. **Increase threshold** in `playwright.config.ts` (current: 0.2)
2. **Check Docker rendering**: Run tests in a Linux container matching CI
3. **Review diff artifacts**: CI uploads diff images - inspect them
4. **Regenerate on CI**: Sometimes regenerating snapshots on same platform helps

### Flaky Snapshot Tests

**Cause**: Non-deterministic UI elements (animations, loading states, timestamps)

**Solutions**:
1. **Wait for stability**:
   ```typescript
   await page.waitForTimeout(1000); // Allow animations to settle
   ```
2. **Disable animations**:
   ```typescript
   await expect(page).toHaveScreenshot("name.png", {
     animations: "disabled",
   });
   ```
3. **Mock dynamic data**: Ensure timestamps, IDs, etc. are deterministic

### Snapshots Out of Sync

**Cause**: Multiple developers updating UI without coordinating snapshots

**Solutions**:
1. **Merge main frequently**: `git pull origin main`
2. **Regenerate snapshots** after merging:
   ```bash
   ./scripts/update-ui-snapshots.sh
   ```
3. **Communicate**: Note in PR if you're updating snapshots

### Script Fails: "Operate UI not running"

**Cause**: operate-ui server not accessible

**Solutions**:
1. **Start Operate UI**:
   ```bash
   OPERATE_UI_FLAGS=contracts-browser,canvas-ui cargo run -p operate-ui
   ```
2. **Check port**: Default is 3000, override with `BASE_URL=http://localhost:PORT`
3. **Verify health**: `curl http://localhost:3000/api/runs`

## Best Practices

### Snapshot Hygiene

- **Commit snapshots with code**: Never commit UI changes without updated snapshots
- **Review carefully**: Snapshots are code - review them like you would any diff
- **Keep deterministic**: Avoid timestamps, random IDs, or animations in snapshots
- **Minimize noise**: Only snapshot critical UI areas, not entire pages if unnecessary

### CI Expectations

- **Non-blocking**: `ui-snapshots` job is advisory (not required to merge)
- **Monitor failures**: Even if not blocking, investigate failures
- **Flake tolerance**: CI retries 2x, but persistent failures need investigation
- **Artifact review**: Check uploaded diff images on failures

### Performance

- **Snapshot updates are slow**: Expect 1-2 minutes for full update
- **Parallel tests**: Playwright runs tests in parallel by default
- **CI caching**: Playwright browsers and npm modules are cached
- **Incremental**: Use `--grep` to update specific snapshots when iterating

## Quick Reference

### Common Commands

```bash
# Update all snapshots
./scripts/update-ui-snapshots.sh

# Run snapshot tests
cd operate-ui/playwright && npm test --grep "visual snapshot"

# View Playwright report
cd operate-ui/playwright && npx playwright show-report

# Start prerequisites
make up  # NATS
OPERATE_UI_FLAGS=contracts-browser,canvas-ui cargo run -p operate-ui  # UI
./examples/seed/seed_preview.sh  # Seed data
```

### File Locations

| Item | Location |
|------|----------|
| Snapshots | `operate-ui/tests/__artifacts__/snapshots/` |
| Tests | `operate-ui/playwright/tests/*.spec.ts` |
| Config | `operate-ui/playwright/playwright.config.ts` |
| Update script | `scripts/update-ui-snapshots.sh` |
| CI workflow | `.github/workflows/ci.yml` (job: `ui-snapshots`) |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BASE_URL` | Operate UI URL | `http://localhost:3000` |
| `NATS_URL` | NATS connection URL | `nats://127.0.0.1:4222` |
| `UPDATE_SNAPSHOTS` | Force update all snapshots | `false` |
| `OPERATE_UI_FLAGS` | Feature flags for UI | (required: `contracts-browser,canvas-ui`) |

## See Also

- [AGENTS.md](../../AGENTS.md) - Agent workflow guidance
- [Playwright Documentation](https://playwright.dev/docs/test-snapshots)
- [Operate UI README](../operate-ui/README.md)
- [Canvas UI Documentation](../canvas-ui.md)
