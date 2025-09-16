import { test, expect } from "@playwright/test";

test("runs page surfaces JetStream status", async ({ page, baseURL }) => {
  await page.goto(`${baseURL}/runs`);

  const statusBadge = page.locator(".status-indicator").first();
  await expect(statusBadge).toContainText(/JetStream (Connected|Unavailable)/);

  const badgeText = (await statusBadge.textContent()) || "";
  if (badgeText.includes("JetStream Unavailable")) {
    await expect(page.locator(".alert.alert-warning")).toContainText(
      "JetStream is not available. Unable to retrieve runs from the event store."
    );
  }

  // Poll for the API response to be properly shaped (handles race conditions)
  await expect
    .poll(async () => {
      const r = await page.request.get(`${baseURL}/api/runs`, { timeout: 15000 });
      if (r.status() === 502) {
        const err = await r.json().catch(() => ({}));
        if (err && typeof err.error === 'string') {
          return 0;
        }
        return -1;
      }
      if (!r.ok()) return -1;
      const j = await r.json().catch(() => ({}));
      // Accept three shapes:
      // 1) direct array:    [ {...}, {...} ]
      if (Array.isArray(j)) return j.length;
      // 2) wrapped:         { runs: [ {...} ] }
      if (j && Array.isArray((j as any).runs)) return (j as any).runs.length;
      // 3) error:           { error: "…" } => treat as 0 (no runs yet)
      if (j && (j as any).error) return 0;
      return -1;
    }, { timeout: 30000, intervals: [500, 1000, 2000] })
    .not.toBe(-1);
});
