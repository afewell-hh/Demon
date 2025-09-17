import { test, expect, Page } from '@playwright/test';
import { seedRun, appendEvents, waitForOperateUI, SeedEvent } from './support/jetstream';

const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:3000';
const allowlistedApprover = process.env.APPROVER_EMAIL || 'tester@example.com';
const ritualId = 'playwright-approvals';
const tenantId = 'playwright';

const startedEvent = (ts: string): SeedEvent => ({
  event: 'ritual.started:v1',
  ts,
});

const completedEvent = (ts: string): SeedEvent => ({
  event: 'ritual.completed:v1',
  ts,
});

const approvalRequested = (ts: string, gateId: string, requester = 'alice@example.com', reason = 'Deploy to production'): SeedEvent => ({
  event: 'approval.requested:v1',
  ts,
  data: {
    gateId,
    requester,
    reason,
  },
});

const approvalGranted = (ts: string, gateId: string, approver: string, note?: string): SeedEvent => ({
  event: 'approval.granted:v1',
  ts,
  data: {
    gateId,
    approver,
    ...(note ? { note } : {}),
  },
});

const approvalDenied = (ts: string, gateId: string, approver: string, reason: string): SeedEvent => ({
  event: 'approval.denied:v1',
  ts,
  data: {
    gateId,
    approver,
    reason,
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
  return `playwright-${slug}`;
}

test.describe('Approval Actions', () => {
  test('approval buttons are shown only for pending approvals', async ({ page }) => {
    const runId = buildRunId('pending');
  await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-01T10:00:00Z'),
        approvalRequested('2024-01-01T10:01:00Z', 'gate-123'),
      ],
    });

    await loadRun(page, runId);

    const approvalsCard = page.locator('div.card').filter({
      has: page.locator('h3:has-text("Approvals")'),
    });

    await expect(approvalsCard).toBeVisible();
    await expect(page.locator('#approval-status')).toContainText('Pending');
    await expect(page.locator('#approval-actions')).toBeVisible();
    await expect(page.locator('#grant-approval-btn')).toBeVisible();
    await expect(page.locator('#deny-approval-btn')).toBeVisible();
    await expect(page.locator('#approver-email')).toBeVisible();
    await expect(page.locator('#approval-note')).toBeVisible();
  });

  test('approval buttons are hidden for completed approvals', async ({ page }) => {
    const runId = buildRunId('granted');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-01-01T10:00:00Z'),
        approvalRequested('2024-01-01T10:01:00Z', 'gate-123'),
        approvalGranted('2024-01-01T10:05:00Z', 'gate-123', 'bob@example.com', 'Looks good'),
        completedEvent('2024-01-01T10:06:00Z'),
      ],
    });

    await loadRun(page, runId);

    const approvalsCard = page.locator('div.card').filter({
      has: page.locator('h3:has-text("Approvals")'),
    });

    await expect(approvalsCard).toBeVisible();
    await expect(page.locator('#approval-status')).toContainText('Granted');
    await expect(page.locator('#approval-actions')).toBeHidden();
  });

  test('grant approval flow with validation', async ({ page }) => {
    const runId = buildRunId('grant-flow');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-02-01T08:00:00Z'),
        approvalRequested('2024-02-01T08:01:00Z', 'gate-456'),
      ],
    });

    await loadRun(page, runId);

    await page.click('#grant-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please enter your email address');

    await page.fill('#approver-email', 'invalid-email');
    await page.click('#grant-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please enter a valid email address');

    await page.fill('#approver-email', allowlistedApprover);
    await page.fill('#approval-note', 'Test approval note');
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('Approval granted successfully', {
      timeout: 12000,
    });
    await expect(page.locator('#grant-approval-btn')).toBeDisabled();
    await expect(page.locator('#deny-approval-btn')).toBeDisabled();
    await expect(page.locator('#approval-status')).toContainText('Granted', { timeout: 12000 });
    await expect(page.locator('#approval-actions')).toBeHidden({ timeout: 12000 });
  });

  test('deny approval flow with required reason', async ({ page }) => {
    const runId = buildRunId('deny-flow');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-03-01T08:00:00Z'),
        approvalRequested('2024-03-01T08:01:00Z', 'gate-789'),
      ],
    });

    await loadRun(page, runId);

    await page.fill('#approver-email', allowlistedApprover);
    await page.click('#deny-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please provide a reason for denying this approval');

    await page.fill('#approval-note', 'Security concerns not addressed');
    await page.click('#deny-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('Approval denied successfully', {
      timeout: 12000,
    });
    await expect(page.locator('#approval-status')).toContainText('Denied', { timeout: 12000 });
    await expect(page.locator('#approval-actions')).toBeHidden({ timeout: 12000 });
  });

  test('handles authorization error (403)', async ({ page }) => {
    const runId = buildRunId('403');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-04-01T08:00:00Z'),
        approvalRequested('2024-04-01T08:01:00Z', 'gate-403'),
      ],
    });

    await loadRun(page, runId);

    await page.fill('#approver-email', 'unauthorized@example.com');
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('You are not authorized to approve this request');
    await expect(page.locator('#grant-approval-btn')).toBeEnabled();
  });

  test('handles conflict error (409)', async ({ page }) => {
    const runId = buildRunId('409');
    const sseEndpoint = `${baseUrl}/api/runs/${runId}/events/stream`;
    await page.route(sseEndpoint, route => route.abort());
    try {
      await seedRun({
        runId,
        ritualId,
        tenantId,
        events: [
          startedEvent('2024-05-01T08:00:00Z'),
          approvalRequested('2024-05-01T08:01:00Z', 'gate-409'),
        ],
      });

      await loadRun(page, runId);

      await appendEvents({
        runId,
        ritualId,
        tenantId,
        events: [
          approvalDenied('2024-05-01T09:00:00Z', 'gate-409', allowlistedApprover, 'Manual override'),
        ],
      });

      await page.fill('#approver-email', allowlistedApprover);
      await page.click('#grant-approval-btn');

      await expect(page.locator('#approval-toast')).toContainText('gate already resolved', {
        timeout: 12000,
      });
      await expect(page.locator('#approval-status')).toContainText('Denied', { timeout: 12000 });
      await expect(page.locator('#approval-actions')).toBeHidden({ timeout: 12000 });
    } finally {
      await page.unroute(sseEndpoint);
    }
  });

  test('handles network errors gracefully', async ({ page }) => {
    const runId = buildRunId('network');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-06-01T08:00:00Z'),
        approvalRequested('2024-06-01T08:01:00Z', 'gate-network'),
      ],
    });

    await loadRun(page, runId);

    await page.route(`${baseUrl}/api/approvals/${runId}/gate-network/grant`, route => {
      route.abort('failed');
    });

    await page.fill('#approver-email', allowlistedApprover);
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('Network error occurred');
    await expect(page.locator('#grant-approval-btn')).toBeEnabled();

    await page.unroute(`${baseUrl}/api/approvals/${runId}/gate-network/grant`);
  });

  test('supports enter key navigation', async ({ page }) => {
    const runId = buildRunId('keyboard');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-07-01T08:00:00Z'),
        approvalRequested('2024-07-01T08:01:00Z', 'gate-keyboard'),
      ],
    });

    await loadRun(page, runId);

    await page.fill('#approver-email', allowlistedApprover);
    await page.press('#approver-email', 'Enter');
    await expect(page.locator('#approval-note')).toBeFocused();
  });

  test('handles missing CSRF header', async ({ page }) => {
    const runId = buildRunId('csrf');
    await seedRun({
      runId,
      ritualId,
      tenantId,
      events: [
        startedEvent('2024-08-01T08:00:00Z'),
        approvalRequested('2024-08-01T08:01:00Z', 'gate-csrf'),
      ],
    });

    await loadRun(page, runId);

    const endpoint = `${baseUrl}/api/approvals/${runId}/gate-csrf/grant`;
    await page.route(endpoint, async route => {
      const headers = { ...route.request().headers() };
      delete headers['x-requested-with'];
      await route.continue({ headers });
    });

    await page.fill('#approver-email', allowlistedApprover);
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('X-Requested-With header required');

    await page.unroute(endpoint);
  });
});
