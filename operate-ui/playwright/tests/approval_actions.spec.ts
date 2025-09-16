import { test, expect } from '@playwright/test';

// Ensure BASE_URL is set for CI
const baseUrl = process.env.BASE_URL || 'http://127.0.0.1:3000';

test.describe('Approval Actions', () => {
  test.beforeEach(async ({ page }) => {
    // Wait for the server to be ready
    await page.goto(`${baseUrl}/api/runs`, { waitUntil: 'networkidle' });
  });

  test('approval buttons are shown only for pending approvals', async ({ page }) => {
    // Create a mock run detail response with pending approval
    await page.route(`${baseUrl}/api/runs/test-pending-approval`, async route => {
      const mockRun = {
        runId: 'test-pending-approval',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:00:00Z',
            event: 'ritual.started:v1',
            extra: {}
          },
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-123',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.goto(`${baseUrl}/runs/test-pending-approval`);

    // Approval section should be visible
    await expect(page.locator('.card:has-text("Approvals")')).toBeVisible();
    await expect(page.locator('#approval-status')).toContainText('Pending');

    // Approval actions should be visible for pending approvals
    await expect(page.locator('#approval-actions')).toBeVisible();
    await expect(page.locator('#grant-approval-btn')).toBeVisible();
    await expect(page.locator('#deny-approval-btn')).toBeVisible();
    await expect(page.locator('#approver-email')).toBeVisible();
    await expect(page.locator('#approval-note')).toBeVisible();
  });

  test('approval buttons are hidden for completed approvals', async ({ page }) => {
    // Create a mock run detail response with granted approval
    await page.route(`${baseUrl}/api/runs/test-granted-approval`, async route => {
      const mockRun = {
        runId: 'test-granted-approval',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:00:00Z',
            event: 'ritual.started:v1',
            extra: {}
          },
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-123',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          },
          {
            ts: '2024-01-01T10:05:00Z',
            event: 'approval.granted:v1',
            extra: {
              gateId: 'gate-123',
              approver: 'bob@example.com',
              note: 'Looks good'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.goto(`${baseUrl}/runs/test-granted-approval`);

    // Approval section should be visible but showing granted status
    await expect(page.locator('.card:has-text("Approvals")')).toBeVisible();
    await expect(page.locator('#approval-status')).toContainText('Granted');

    // Approval actions should NOT be visible for completed approvals
    await expect(page.locator('#approval-actions')).not.toBeVisible();
  });

  test('grant approval flow with validation', async ({ page }) => {
    // Mock the pending approval run
    await page.route(`${baseUrl}/api/runs/test-approval-grant`, async route => {
      const mockRun = {
        runId: 'test-approval-grant',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-456',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    // Mock the grant approval API
    await page.route(`${baseUrl}/api/approvals/test-approval-grant/gate-456/grant`, async route => {
      const request = route.request();
      const body = request.postDataJSON();

      // Verify required headers
      expect(request.headers()['x-requested-with']).toBe('XMLHttpRequest');
      expect(request.headers()['content-type']).toBe('application/json');

      // Verify body structure
      expect(body.approver).toBe('tester@example.com');
      expect(body.note).toBe('Test approval note');

      await route.fulfill({
        json: {
          event: 'approval.granted:v1',
          ts: '2024-01-01T10:05:00Z',
          runId: 'test-approval-grant',
          gateId: 'gate-456',
          approver: 'tester@example.com',
          note: 'Test approval note'
        }
      });
    });

    await page.goto(`${baseUrl}/runs/test-approval-grant`);

    // Try to grant without email - should show validation error
    await page.click('#grant-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please enter your email address');

    // Enter invalid email - should show validation error
    await page.fill('#approver-email', 'invalid-email');
    await page.click('#grant-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please enter a valid email address');

    // Enter valid email and note
    await page.fill('#approver-email', 'tester@example.com');
    await page.fill('#approval-note', 'Test approval note');

    // Grant the approval
    await page.click('#grant-approval-btn');

    // Should show success message
    await expect(page.locator('#approval-toast')).toContainText('Approval granted successfully');

    // Buttons should be disabled during request
    await expect(page.locator('#grant-approval-btn')).toBeDisabled();
    await expect(page.locator('#deny-approval-btn')).toBeDisabled();

    // Status should update to Granted
    await expect(page.locator('#approval-status')).toContainText('Granted');

    // Actions should be hidden
    await expect(page.locator('#approval-actions')).not.toBeVisible();
  });

  test('deny approval flow with required reason', async ({ page }) => {
    // Mock the pending approval run
    await page.route(`${baseUrl}/api/runs/test-approval-deny`, async route => {
      const mockRun = {
        runId: 'test-approval-deny',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-789',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    // Mock the deny approval API
    await page.route(`${baseUrl}/api/approvals/test-approval-deny/gate-789/deny`, async route => {
      const request = route.request();
      const body = request.postDataJSON();

      expect(body.approver).toBe('tester@example.com');
      expect(body.reason).toBe('Security concerns not addressed');

      await route.fulfill({
        json: {
          event: 'approval.denied:v1',
          ts: '2024-01-01T10:05:00Z',
          runId: 'test-approval-deny',
          gateId: 'gate-789',
          approver: 'tester@example.com',
          reason: 'Security concerns not addressed'
        }
      });
    });

    await page.goto(`${baseUrl}/runs/test-approval-deny`);

    // Enter email but no reason
    await page.fill('#approver-email', 'tester@example.com');

    // Try to deny without reason - should show validation error
    await page.click('#deny-approval-btn');
    await expect(page.locator('#approval-toast')).toContainText('Please provide a reason for denying this approval');

    // Enter reason
    await page.fill('#approval-note', 'Security concerns not addressed');

    // Deny the approval
    await page.click('#deny-approval-btn');

    // Should show success message
    await expect(page.locator('#approval-toast')).toContainText('Approval denied successfully');

    // Status should update to Denied
    await expect(page.locator('#approval-status')).toContainText('Denied');

    // Actions should be hidden
    await expect(page.locator('#approval-actions')).not.toBeVisible();
  });

  test('handles authorization error (403)', async ({ page }) => {
    await page.route(`${baseUrl}/api/runs/test-approval-403`, async route => {
      const mockRun = {
        runId: 'test-approval-403',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-403',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.route(`${baseUrl}/api/approvals/test-approval-403/gate-403/grant`, async route => {
      await route.fulfill({
        status: 403,
        json: { error: 'approver not allowed' }
      });
    });

    await page.goto(`${baseUrl}/runs/test-approval-403`);

    await page.fill('#approver-email', 'unauthorized@example.com');
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('You are not authorized to approve this request');
    await expect(page.locator('#grant-approval-btn')).not.toBeDisabled();
  });

  test('handles conflict error (409)', async ({ page }) => {
    await page.route(`${baseUrl}/api/runs/test-approval-409`, async route => {
      const mockRun = {
        runId: 'test-approval-409',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-409',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.route(`${baseUrl}/api/approvals/test-approval-409/gate-409/grant`, async route => {
      await route.fulfill({
        status: 409,
        json: {
          error: 'gate already resolved',
          state: 'granted'
        }
      });
    });

    await page.goto(`${baseUrl}/runs/test-approval-409`);

    await page.fill('#approver-email', 'tester@example.com');
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('gate already resolved');
    await expect(page.locator('#approval-status')).toContainText('Granted');
  });

  test('handles network errors gracefully', async ({ page }) => {
    await page.route(`${baseUrl}/api/runs/test-approval-network`, async route => {
      const mockRun = {
        runId: 'test-approval-network',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-network',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.route(`${baseUrl}/api/approvals/test-approval-network/gate-network/grant`, async route => {
      await route.abort('failed');
    });

    await page.goto(`${baseUrl}/runs/test-approval-network`);

    await page.fill('#approver-email', 'tester@example.com');
    await page.click('#grant-approval-btn');

    await expect(page.locator('#approval-toast')).toContainText('Network error occurred');
    await expect(page.locator('#grant-approval-btn')).not.toBeDisabled();
  });

  test('supports enter key navigation', async ({ page }) => {
    await page.route(`${baseUrl}/api/runs/test-approval-keyboard`, async route => {
      const mockRun = {
        runId: 'test-approval-keyboard',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-keyboard',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.goto(`${baseUrl}/runs/test-approval-keyboard`);

    // Type email and press Enter - should focus note field
    await page.fill('#approver-email', 'tester@example.com');
    await page.press('#approver-email', 'Enter');

    // Note field should be focused
    await expect(page.locator('#approval-note')).toBeFocused();
  });

  test('handles missing CSRF header', async ({ page }) => {
    await page.route(`${baseUrl}/api/runs/test-approval-csrf`, async route => {
      const mockRun = {
        runId: 'test-approval-csrf',
        ritualId: 'test-ritual',
        events: [
          {
            ts: '2024-01-01T10:01:00Z',
            event: 'approval.requested:v1',
            extra: {
              gateId: 'gate-csrf',
              requester: 'alice@example.com',
              reason: 'Deploy to production'
            }
          }
        ]
      };
      await route.fulfill({ json: mockRun });
    });

    await page.route(`${baseUrl}/api/approvals/test-approval-csrf/gate-csrf/grant`, async route => {
      const headers = route.request().headers();
      if (!headers['x-requested-with']) {
        await route.fulfill({
          status: 400,
          json: { error: 'X-Requested-With header required' }
        });
      } else {
        await route.fulfill({ json: { success: true } });
      }
    });

    await page.goto(`${baseUrl}/runs/test-approval-csrf`);

    // Normal flow should work (includes CSRF header)
    await page.fill('#approver-email', 'tester@example.com');
    await page.click('#grant-approval-btn');

    // Should not get CSRF error since the JavaScript includes the header
    await expect(page.locator('#approval-toast')).not.toContainText('X-Requested-With header required');
  });
});