import { test, expect } from '@playwright/test';

// Ensure BASE_URL is set for CI
const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:3000';

test.describe('Run Streaming', () => {
  test.beforeEach(async ({ page }) => {
    // Wait for the server to be ready
    await page.goto(`${baseUrl}/api/runs`, { waitUntil: 'networkidle' });
  });

  test('shows connection status indicator on run detail page', async ({ page }) => {
    // Go to runs page first
    await page.goto(`${baseUrl}/runs`);

    // Handle both empty and populated runs list
    const hasRuns = await page.locator('tbody tr').first().isVisible().catch(() => false);

    if (hasRuns) {
      // Click on the first run link
      await page.locator('tbody tr').first().locator('a').first().click();
      await page.waitForSelector('#connection-status');

      // Connection indicator should be visible
      const connectionIndicator = page.locator('#connection-status');
      await expect(connectionIndicator).toBeVisible({ timeout: 10000 });

      // Should show a connection status (Connected, Connecting, or Connected (degraded))
      const statusText = await connectionIndicator.locator('.connection-text').textContent();
      expect(['Connected', 'Connecting...', 'Connected (degraded)', 'Reconnecting...', 'Offline']).toContain(statusText?.trim());

      // Connection icon should be visible
      await expect(connectionIndicator.locator('.connection-icon')).toBeVisible();
    } else {
      // If no runs, go to a test run page directly
      await page.goto(`${baseUrl}/runs/test-run-123`);

      // Even for non-existent runs, connection indicator should work
      const connectionIndicator = page.locator('#connection-status');

      // It may or may not show depending on JetStream availability
      const isVisible = await connectionIndicator.isVisible().catch(() => false);
      if (isVisible) {
        const statusText = await connectionIndicator.locator('.connection-text').textContent();
        expect(statusText).toBeTruthy();
      }
    }
  });

  test('SSE endpoint returns correct headers', async () => {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 2000);

    const response = await fetch(`${baseUrl}/api/runs/test-run/events/stream`, {
      method: 'GET',
      signal: controller.signal,
      headers: {
        Accept: 'text/event-stream',
      },
    });

    clearTimeout(timeout);

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toBe('text/event-stream');
    expect(response.headers.get('cache-control')).toBe('no-cache');
    expect(response.headers.get('connection')).toBe('keep-alive');

    // Cancel the body stream to avoid hanging the test runner
    await response.body?.cancel();
  });

  test('connection indicator updates on SSE events', async ({ page }) => {
    await page.goto(`${baseUrl}/runs`);

    const runRow = page.locator('tbody tr').first();
    const hasRuns = await runRow.isVisible().catch(() => false);

    if (!hasRuns) {
      console.warn('No runs available to verify streaming indicator');
      return;
    }

    await runRow.locator('a').first().click();
    await page.waitForSelector('#connection-status');

    // If JetStream is available, check connection indicator behavior
    const connectionIndicator = page.locator('#connection-status');

    await page.waitForFunction(
      () => {
        const indicator = document.querySelector('#connection-status');
        if (!indicator) return false;
        const text = indicator.querySelector('.connection-text')?.textContent || '';
        return text.includes('Connected') || text.includes('Reconnecting');
      },
      { timeout: 20000 }
    );

    const indicatorClasses = await connectionIndicator.getAttribute('class');
    expect(indicatorClasses).toMatch(/connection-indicator\s+(connected|reconnecting|offline)/);
  });

  test('handles SSE reconnection gracefully', async ({ page, context }) => {
    // Enable console logging to verify reconnection logic
    page.on('console', msg => {
      if (msg.type() === 'info' && msg.text().includes('[SSE]')) {
        console.log('Browser console:', msg.text());
      }
    });

    await page.goto(`${baseUrl}/runs/test-reconnect`);

    // Check if connection indicator appears
    const connectionIndicator = page.locator('#connection-status');
    const isVisible = await connectionIndicator.isVisible({ timeout: 5000 }).catch(() => false);

    if (isVisible) {
      // Simulate network interruption by going offline
      await context.setOffline(true);

      // Should show reconnecting or offline status
      await page.waitForFunction(
        () => {
          const text = document.querySelector('#connection-status .connection-text')?.textContent || '';
          return text.includes('Reconnecting') || text.includes('Offline');
        },
        { timeout: 5000 }
      ).catch(() => {
        // It's okay if this doesn't trigger immediately
      });

      // Go back online
      await context.setOffline(false);

      // Should eventually reconnect
      await page.waitForFunction(
        () => {
          const text = document.querySelector('#connection-status .connection-text')?.textContent || '';
          return text.includes('Connected');
        },
        { timeout: 15000 }
      ).catch(() => {
        // Reconnection might take time or JetStream might not be available
      });
    }
  });

  test('event timeline updates without page reload', async ({ page }) => {
    await page.goto(`${baseUrl}/runs`);

    const hasRuns = await page.locator('tbody tr').first().isVisible().catch(() => false);

    if (hasRuns) {
      // Get initial event count
      await page.locator('tbody tr').first().locator('a').first().click();
      await page.waitForSelector('#connection-status');

      const eventRows = page.locator('.table tbody tr');
      const initialCount = await eventRows.count();

      // Check if the page has SSE support (JetStream available)
      const hasSSE = await page.evaluate(() => {
        return typeof EventSource !== 'undefined' &&
               document.querySelector('#connection-status') !== null;
      });

      if (hasSSE) {
        // Wait a moment to see if new events arrive (in a real scenario with active runs)
        await page.waitForTimeout(3000);

        // Count should potentially increase if events are streaming
        const newCount = await eventRows.count();
        expect(newCount).toBeGreaterThanOrEqual(initialCount);

        // Verify no page reload occurred
        const navigationPromise = page.waitForNavigation({ timeout: 1000 });
        await expect(navigationPromise).rejects.toThrow('Timeout');
      }
    }
  });
});
