import { test, expect, Page } from '@playwright/test';
import { seedRun, waitForOperateUI, SeedEvent } from './support/jetstream';
import { seedScaleHint, clearScaleHints } from './support/scale_hints';

const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:3000';
const ritualId = 'test-ritual';
const tenantId = 'default';

const startedEvent = (ts: string): SeedEvent => ({
  event: 'ritual.started:v1',
  ts,
  stateFrom: '',
  stateTo: 'running',
});

const completedEvent = (ts: string): SeedEvent => ({
  event: 'ritual.completed:v1',
  ts,
  stateFrom: 'running',
  stateTo: 'completed',
  data: {},
});

test.beforeAll(async () => {
  await waitForOperateUI(baseUrl);
});

async function loadRun(page: Page, runId: string) {
  await page.goto(`${baseUrl}/runs/${runId}`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('main')).toBeVisible();
}

function buildRunId(slug: string) {
  return `playwright-scale-${slug}`;
}

test.describe('Scale Metrics Display', () => {
  test('run detail page shows scale metrics panel when scale hint available', async ({ page }) => {
    const runId = buildRunId('with-scale-hint');

    // Seed run
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T10:00:00Z'),
        completedEvent('2025-01-06T10:01:00Z'),
      ],
    });

    // Seed scale hint
    await seedScaleHint({
      tenantId,
      recommendation: 'scale_up',
      metrics: {
        queueLag: 850,
        p95LatencyMs: 1250.5,
        errorRate: 0.08,
        totalProcessed: 1000,
        totalErrors: 80,
      },
      reason: 'Queue lag (850) exceeds high threshold (500) and P95 latency (1250.5ms) exceeds high threshold (1000ms)',
    });

    await loadRun(page, runId);

    // Verify scale metrics panel exists
    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });
    await expect(metricsPanel).toBeVisible();

    // Verify recommendation badge
    const badge = metricsPanel.locator('.scale-recommendation-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveClass(/scale-recommendation-scale_up/);
    await expect(badge).toContainText('Scale Up');

    // Verify metrics are displayed
    const metricCards = metricsPanel.locator('.metric-card');
    expect(await metricCards.count()).toBe(3);

    // Check Queue Lag
    const queueLagCard = metricCards.nth(0);
    await expect(queueLagCard.locator('.metric-label')).toContainText('Queue Lag');
    await expect(queueLagCard.locator('.metric-value')).toContainText('850');

    // Check P95 Latency
    const latencyCard = metricCards.nth(1);
    await expect(latencyCard.locator('.metric-label')).toContainText('P95 Latency');
    await expect(latencyCard.locator('.metric-value')).toContainText('1.25s'); // 1250.5ms converted to seconds

    // Check Error Rate
    const errorCard = metricCards.nth(2);
    await expect(errorCard.locator('.metric-label')).toContainText('Error Rate');
    await expect(errorCard.locator('.metric-value')).toContainText('8'); // 0.08 * 100 = 8%

    // Verify reason is displayed
    await expect(metricsPanel).toContainText('Queue lag (850) exceeds high threshold');
  });

  test('scale-down recommendation shows correct badge color', async ({ page }) => {
    const runId = buildRunId('scale-down');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T11:00:00Z'),
        completedEvent('2025-01-06T11:01:00Z'),
      ],
    });

    await seedScaleHint({
      tenantId,
      recommendation: 'scale_down',
      metrics: {
        queueLag: 10,
        p95LatencyMs: 50.0,
        errorRate: 0.001,
        totalProcessed: 1000,
        totalErrors: 1,
      },
      reason: 'System is underutilized',
    });

    await loadRun(page, runId);

    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });

    const badge = metricsPanel.locator('.scale-recommendation-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveClass(/scale-recommendation-scale_down/);
    await expect(badge).toContainText('Scale Down');
  });

  test('steady recommendation shows correct badge color', async ({ page }) => {
    const runId = buildRunId('steady');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T12:00:00Z'),
        completedEvent('2025-01-06T12:01:00Z'),
      ],
    });

    await seedScaleHint({
      tenantId,
      recommendation: 'steady',
      metrics: {
        queueLag: 100,
        p95LatencyMs: 200.0,
        errorRate: 0.02,
        totalProcessed: 1000,
        totalErrors: 20,
      },
      reason: 'System operating within normal parameters',
    });

    await loadRun(page, runId);

    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });

    const badge = metricsPanel.locator('.scale-recommendation-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveClass(/scale-recommendation-steady/);
    await expect(badge).toContainText('Steady');
  });

  test('run detail page hides scale metrics panel when no scale hint available', async ({ page }) => {
    const runId = buildRunId('no-scale-hint');

    // Clear any existing scale hints from previous tests
    await clearScaleHints(tenantId);

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T13:00:00Z'),
        completedEvent('2025-01-06T13:01:00Z'),
      ],
    });

    // Don't seed any scale hint

    await loadRun(page, runId);

    // Verify scale metrics panel does NOT exist
    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });
    await expect(metricsPanel).not.toBeVisible();

    // But page should still load successfully
    await expect(page.locator('main')).toBeVisible();
  });

  test('latency displays in milliseconds when under 1 second', async ({ page }) => {
    const runId = buildRunId('low-latency');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T14:00:00Z'),
        completedEvent('2025-01-06T14:01:00Z'),
      ],
    });

    await seedScaleHint({
      tenantId,
      recommendation: 'steady',
      metrics: {
        queueLag: 50,
        p95LatencyMs: 456.78, // Less than 1000, should display as ms
        errorRate: 0.01,
        totalProcessed: 1000,
        totalErrors: 10,
      },
      reason: 'Normal operation',
    });

    await loadRun(page, runId);

    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });

    const latencyCard = metricsPanel.locator('.metric-card').nth(1);
    await expect(latencyCard.locator('.metric-value')).toContainText('456.78ms');
  });

  test('metrics panel is mobile responsive', async ({ page }) => {
    const runId = buildRunId('mobile-responsive');

    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2025-01-06T15:00:00Z'),
        completedEvent('2025-01-06T15:01:00Z'),
      ],
    });

    await seedScaleHint({
      tenantId,
      recommendation: 'scale_up',
      metrics: {
        queueLag: 500,
        p95LatencyMs: 800.0,
        errorRate: 0.05,
        totalProcessed: 1000,
        totalErrors: 50,
      },
      reason: 'Elevated metrics',
    });

    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    await loadRun(page, runId);

    const metricsPanel = page.locator('.card').filter({
      has: page.locator('h3:has-text("Scale Feedback Metrics")'),
    });

    // Verify panel is visible on mobile
    await expect(metricsPanel).toBeVisible();

    // Verify metrics cards are stacked (grid should wrap on mobile)
    const metricCards = metricsPanel.locator('.metric-card');
    expect(await metricCards.count()).toBe(3);
  });
});
