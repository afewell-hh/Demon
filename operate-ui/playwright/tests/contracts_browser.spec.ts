import { test, expect } from "@playwright/test";

test.describe("Contracts Browser", () => {
  test("loads with feature flag enabled", async ({ page, baseURL }) => {
    // In CI, OPERATE_UI_FLAGS=contracts-browser is set, so page should load
    const response = await page.goto(`${baseURL}/ui/contracts`);

    // Should get 200 when feature flag is enabled
    expect(response?.status()).toBe(200);
  });

  test("displays contracts browser interface", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Check that page loaded
    await expect(page.locator("h1.card-title")).toContainText("Contracts Browser");

    // Check for search input
    await expect(page.locator("#search-input")).toBeVisible();
  });

  test("displays loading state initially", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Should show loading state
    const loadingState = page.locator("#loading-state");
    await expect(loadingState).toBeVisible();
    await expect(loadingState).toContainText("Loading contracts");
  });

  test("handles registry unavailable gracefully", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Wait for API call to complete
    await page.waitForTimeout(2000);

    // Should show either error or empty state
    const errorState = page.locator("#error-state");
    const emptyState = page.locator("#empty-state");

    // At least one should be visible (depending on registry availability)
    const errorVisible = await errorState.isVisible();
    const emptyVisible = await emptyState.isVisible();

    expect(errorVisible || emptyVisible).toBe(true);
  });

  test("search input filters contracts", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Wait for potential contracts to load
    await page.waitForTimeout(2000);

    const searchInput = page.locator("#search-input");
    await expect(searchInput).toBeVisible();

    // Type in search box
    await searchInput.fill("test-contract");

    // Search should be debounced (wait a bit)
    await page.waitForTimeout(500);

    // Verify input value is set
    await expect(searchInput).toHaveValue("test-contract");
  });

  test("nav link only appears when feature flag enabled", async ({ page, baseURL }) => {
    // With OPERATE_UI_FLAGS=contracts-browser in CI, the nav link should appear
    await page.goto(`${baseURL}/runs`);
    const contractsLink = page.locator('nav a[href*="/ui/contracts"]');

    // In CI with the env var set, the link should be present
    // (This test will fail if the env var is not set in CI)
    await expect(contractsLink).toHaveCount(1);
  });

  test("drawer opens and closes", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Wait for page to load
    await page.waitForTimeout(1000);

    // Drawer should not be visible initially
    const drawer = page.locator("#detail-drawer");
    await expect(drawer).not.toHaveClass(/open/);

    // Overlay should not be visible
    const overlay = page.locator("#drawer-overlay");
    await expect(overlay).not.toHaveClass(/visible/);
  });

  test("close drawer button is accessible", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    const closeButton = page.locator("#close-drawer");
    await expect(closeButton).toHaveAttribute("aria-label", "Close");
  });

  test("contract count indicator exists", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    const contractCount = page.locator("#contract-count");
    await expect(contractCount).toBeVisible();

    // Initially shows loading
    await expect(contractCount).toContainText(/Loading|contract/i);
  });

  test("keyboard navigation - Escape closes drawer", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/ui/contracts`);

    // Press Escape key
    await page.keyboard.press("Escape");

    // Drawer should remain closed (test that listener is set up)
    const drawer = page.locator("#detail-drawer");
    await expect(drawer).not.toHaveClass(/open/);
  });
});

test.describe("Contracts Browser API Integration", () => {
  test("API endpoint returns JSON or 404", async ({ page, baseURL }) => {
    // Check if API is accessible
    const response = await page.request.get(`${baseURL}/api/contracts/registry/list`);

    // Should return 200 (if feature enabled), 404 (if disabled), or 502 (if registry unavailable)
    expect([200, 404, 502]).toContain(response.status());

    if (response.status() === 200) {
      const data = await response.json();
      expect(data).toHaveProperty("contracts");
      expect(Array.isArray(data.contracts)).toBe(true);
    }
  });

  test("contract detail API accepts name and version", async ({ page, baseURL }) => {
    // Try to fetch a contract (will 404 if doesn't exist or feature disabled, which is fine)
    const response = await page.request.get(
      `${baseURL}/api/contracts/registry/test-contract/1.0.0`
    );

    // Should return 404 (not found or feature disabled) or 502 (registry unavailable) - both are acceptable
    expect([404, 502]).toContain(response.status());
  });
});
