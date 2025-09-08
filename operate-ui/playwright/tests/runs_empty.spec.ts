import { test, expect } from "@playwright/test";

test("runs page shows banner when stream is missing", async ({ page, baseURL }) => {
  await page.goto(`${baseURL}/runs`);
  await expect(page.locator("body")).toContainText(
    "No event stream found. See Runbook: setup."
  );

  const resp = await page.request.get(`${baseURL}/api/runs`);
  expect(resp.status()).toBe(200);
  const json = await resp.json();
  expect(Array.isArray(json.runs)).toBe(true);
});

