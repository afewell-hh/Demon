import { test, expect } from "@playwright/test";

test("runs page shows banner when stream is missing", async ({ page, baseURL }) => {
  await page.goto(`${baseURL}/runs`);
  await expect(page.locator("body")).toContainText(
    "No event stream found. See Runbook: setup."
  );

  // Poll for the API response to be properly shaped (handles race conditions)
  await expect
    .poll(async () => {
      const r = await page.request.get(`${baseURL}/api/runs`, { timeout: 5000 });
      if (!r.ok()) return -1;
      const j = await r.json().catch(() => ({}));
      return Array.isArray(j.runs) ? j.runs.length : -1;
    }, { timeout: 30000, intervals: [500, 1000, 2000] })
    .not.toBe(-1);
});

