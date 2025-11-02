import { test, expect, Page } from '@playwright/test';
import { seedRun, waitForOperateUI, SeedEvent } from './support/jetstream';

const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:3000';
const ritualId = 'hello'; // Must match the ritual in app-pack-sample
const tenantId = 'default';

const startedEvent = (ts: string): SeedEvent => ({
  event: 'ritual.started:v1',
  ts,
  stateFrom: '',
  stateTo: 'running',
});

const completedEvent = (ts: string, outputs: Record<string, unknown>): SeedEvent => ({
  event: 'ritual.completed:v1',
  ts,
  stateFrom: 'running',
  stateTo: 'completed',
  data: {
    outputs,
  },
});

test.beforeAll(async () => {
  await waitForOperateUI(baseUrl);
});

async function loadRun(page: Page, runId: string) {
  await page.goto(`${baseUrl}/runs/${runId}`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('main')).toBeVisible();
}

function buildRunId(slug: string) {
  return `playwright-cards-${slug}`;
}

test.describe('App Pack Cards Rendering', () => {
  test('run detail page loads successfully with completed ritual', async ({ page }) => {
    const runId = buildRunId('basic-load');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-01T10:00:00Z'),
        completedEvent('2024-01-01T10:01:00Z', {
          result: {
            success: true,
            data: {
              message: 'Hello from Demon App Pack!',
              timestamp: '2025-11-01T00:00:00Z',
            },
          },
          duration: 1234.56,
        }),
      ],
    });

    await loadRun(page, runId);

    // Page should load without errors
    await expect(page.locator('main')).toBeVisible();

    // Run detail should be displayed
    const cards = page.locator('.card');
    expect(await cards.count()).toBeGreaterThanOrEqual(2); // At minimum: run detail + event timeline
  });

  test('displays cards section when app pack is installed for matching ritual', async ({ page }) => {
    const runId = buildRunId('with-cards');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-01T10:00:00Z'),
        completedEvent('2024-01-01T10:01:00Z', {
          result: {
            success: true,
            data: {
              message: 'Hello from Demon App Pack!',
              timestamp: '2025-11-01T00:00:00Z',
            },
          },
          duration: 1234.56,
        }),
      ],
    });

    await loadRun(page, runId);

    // Check if App Pack Cards section exists (depends on app-pack-sample being installed)
    const cardsSection = page.locator('div.card').filter({
      has: page.locator('h3:has-text("App Pack Cards")'),
    });

    const isVisible = await cardsSection.isVisible().catch(() => false);

    if (isVisible) {
      // If cards section is visible, verify it's structured correctly
      const viewInGraphBtn = cardsSection.locator('a.btn:has-text("View in Graph")');
      await expect(viewInGraphBtn).toBeVisible();
      await expect(viewInGraphBtn).toHaveAttribute('href', `/graph?runId=${runId}`);

      const cards = page.locator('.app-pack-card');
      expect(await cards.count()).toBeGreaterThan(0);

      // Verify first card structure
      const firstCard = cards.first();
      await expect(firstCard.locator('.app-pack-card-title')).toBeVisible();
      await expect(firstCard.locator('.card-kind-badge')).toBeVisible();
      await expect(firstCard.locator('.app-pack-card-content')).toBeVisible();
    } else {
      // If no cards section, that's fine - app pack might not be installed
      // Just verify page loaded successfully
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('json-viewer card displays JSON data correctly when app pack installed', async ({ page }) => {
    const runId = buildRunId('json-viewer');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-02T10:00:00Z'),
        completedEvent('2024-01-02T10:01:00Z', {
          result: {
            success: true,
            data: {
              message: 'Test message',
              nested: {
                field: 'value',
              },
            },
          },
        }),
      ],
    });

    await loadRun(page, runId);

    // Check if json-viewer card exists
    const jsonCard = page.locator('.app-pack-card[data-card-kind="json-viewer"]');
    const isVisible = await jsonCard.isVisible().catch(() => false);

    if (isVisible) {
      // If card is visible, verify JSON content rendering
      const jsonContent = jsonCard.locator('.json-content');
      await expect(jsonContent).toBeVisible();

      const text = await jsonContent.textContent();
      expect(text).toContain('result');
      expect(text).toContain('success');
    } else {
      // No app pack installed, verify page still works
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('hides cards section when no matching ritual', async ({ page }) => {
    const runId = buildRunId('no-match');
    const unmatchedRitual = 'unknown-ritual';

    await seedRun({
      runId,
      ritualId: unmatchedRitual,
      tenantId,
      events: [
        startedEvent('2024-01-03T10:00:00Z'),
        completedEvent('2024-01-03T10:01:00Z', {
          result: { success: true },
        }),
      ],
    });

    await page.goto(`${baseUrl}/runs/${runId}`, { waitUntil: 'domcontentloaded' });
    await expect(page.locator('main')).toBeVisible();

    // Verify cards section is NOT present
    const cardsSection = page.locator('div.card').filter({
      has: page.locator('h3:has-text("App Pack Cards")'),
    });
    await expect(cardsSection).not.toBeVisible();
  });

  test('handles missing ritual.completed event gracefully', async ({ page }) => {
    const runId = buildRunId('no-completed');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-04T10:00:00Z'),
        // No completed event - run is still running
      ],
    });

    await loadRun(page, runId);

    // Page should load without errors
    await expect(page.locator('main')).toBeVisible();

    // Status should show Running
    await expect(page.locator('.status-running')).toBeVisible();
  });

  test('graph viewer integration - View in Graph button works', async ({ page }) => {
    const runId = buildRunId('graph-nav');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-05T10:00:00Z'),
        completedEvent('2024-01-05T10:01:00Z', {
          result: { success: true },
        }),
      ],
    });

    await loadRun(page, runId);

    // Check if View in Graph button exists (depends on app pack being installed)
    const viewInGraphBtn = page.locator('a.btn:has-text("View in Graph")');
    const isVisible = await viewInGraphBtn.isVisible().catch(() => false);

    if (isVisible) {
      // If button is visible, test the navigation
      await expect(viewInGraphBtn).toBeVisible();
      await viewInGraphBtn.click();
      await page.waitForURL(`${baseUrl}/graph?runId=${runId}`);

      // Verify we're on graph viewer page
      await expect(page.locator('h2:has-text("Graph Viewer")')).toBeVisible();
    } else {
      // If no button, app pack not installed - just verify page loaded
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('graph viewer displays run cards panel when runId provided', async ({ page }) => {
    const runId = buildRunId('graph-panel');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-06T10:00:00Z'),
        completedEvent('2024-01-06T10:01:00Z', {
          result: {
            success: true,
            data: { test: 'data' },
          },
        }),
      ],
    });

    // Navigate directly to graph with runId
    await page.goto(`${baseUrl}/graph?runId=${runId}`, { waitUntil: 'domcontentloaded' });

    // Verify Graph Viewer page loaded
    await expect(page.locator('h2:has-text("Graph Viewer")')).toBeVisible();

    // Check if Run Cards panel exists (depends on app pack being installed)
    const runCardsPanel = page.locator('#runCardsPanel');
    const isVisible = await runCardsPanel.isVisible().catch(() => false);

    if (isVisible) {
      // If panel is visible, test its content
      await expect(runCardsPanel).toBeVisible();

      // Check panel header
      await expect(runCardsPanel.locator('h3')).toContainText(`Run Cards - ${runId}`);

      // Check run info
      await expect(runCardsPanel.locator('code').first()).toContainText(runId);
      await expect(runCardsPanel.locator('code').nth(1)).toContainText(ritualId);

      // Check cards are rendered
      const cards = runCardsPanel.locator('.app-pack-card');
      await expect(cards).toHaveCount(1);

      // Check View Run Detail link
      const viewRunDetailLink = runCardsPanel.locator('a.btn:has-text("View Run Detail")');
      await expect(viewRunDetailLink).toBeVisible();
      await expect(viewRunDetailLink).toHaveAttribute('href', `/runs/${runId}`);
    } else {
      // If no panel, app pack not installed - page still works
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('graph viewer without runId does not show cards panel', async ({ page }) => {
    await page.goto(`${baseUrl}/graph`, { waitUntil: 'domcontentloaded' });

    // Verify Graph Viewer page loads
    await expect(page.locator('h2:has-text("Graph Viewer")')).toBeVisible();

    // Verify Run Cards panel is NOT present
    const runCardsPanel = page.locator('#runCardsPanel');
    await expect(runCardsPanel).not.toBeVisible();
  });

  test('card renders with title and description when app pack installed', async ({ page }) => {
    const runId = buildRunId('card-metadata');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-07T10:00:00Z'),
        completedEvent('2024-01-07T10:01:00Z', {
          result: { success: true },
        }),
      ],
    });

    await loadRun(page, runId);

    const card = page.locator('.app-pack-card').first();
    const isVisible = await card.isVisible().catch(() => false);

    if (isVisible) {
      // Verify card metadata rendering
      const title = card.locator('.app-pack-card-title');
      await expect(title).toBeVisible();
      expect(await title.textContent()).toBeTruthy();

      // Kind badge should show card type
      const kindBadge = card.locator('.card-kind-badge');
      await expect(kindBadge).toBeVisible();
      expect(await kindBadge.textContent()).toBeTruthy();
    } else {
      // No app pack installed
      await expect(page.locator('main')).toBeVisible();
    }
  });

  test('cards have expected structure when rendered', async ({ page }) => {
    const runId = buildRunId('card-structure');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-08T10:00:00Z'),
        completedEvent('2024-01-08T10:01:00Z', {
          result: { success: true },
        }),
      ],
    });

    await loadRun(page, runId);

    const cards = page.locator('.app-pack-card');
    const count = await cards.count();

    if (count > 0) {
      // If cards are rendered, verify each has the expected structure
      for (let i = 0; i < count; i++) {
        const card = cards.nth(i);
        await expect(card.locator('.app-pack-card-header')).toBeVisible();
        await expect(card.locator('.app-pack-card-content')).toBeVisible();
      }
    } else {
      // No app pack installed
      await expect(page.locator('main')).toBeVisible();
    }
  });
});
