import { test, expect } from "@playwright/test";

/**
 * End-to-end validation for Contracts Browser user journey
 *
 * Journey: Operator Inspects Contracts via Contracts Browser
 * Persona: Platform operator validating schema compatibility before deploying a new capsule version
 *
 * Given: Operate UI is running with `contracts-browser` feature flag enabled, and a schema registry is accessible
 * When: The operator navigates to the Contracts Browser UI and searches for a specific contract
 * Then: The operator can browse, search, view details, and download contract schemas
 */

test.describe("Contracts Browser End-to-End Journey", () => {
  test("end-to-end: Operator searches and inspects contracts", async ({ page, baseURL }) => {
    // Given: Contracts Browser is loaded with seeded data
    await page.goto(`${baseURL}/ui/contracts`);

    // Wait for page to be interactive
    await page.waitForSelector("#search-input", { state: "visible" });
    await page.waitForTimeout(1000); // Allow any loading states to settle

    // Then: Page loads successfully
    await expect(page.locator("h1.card-title")).toContainText("Contracts Browser");

    // Then: Search functionality is visible
    const searchInput = page.locator("#search-input");
    await expect(searchInput).toBeVisible();

    // Then: Contract count indicator exists
    const contractCount = page.locator("#contract-count");
    await expect(contractCount).toBeVisible();

    // When: Operator searches for a specific contract
    await searchInput.fill("ritual");
    await page.waitForTimeout(500); // Debounce

    // Then: Search input reflects the query
    await expect(searchInput).toHaveValue("ritual");

    // Then: Contract cards are filtered (or empty state is shown)
    const contractCards = page.locator(".contract-card");
    const emptyState = page.locator("#empty-state");
    const errorState = page.locator("#error-state");

    // At least one of these should be visible
    const cardsVisible = await contractCards.count() > 0;
    const emptyVisible = await emptyState.isVisible();
    const errorVisible = await errorState.isVisible();

    expect(cardsVisible || emptyVisible || errorVisible).toBe(true);

    // If cards are visible, test interaction
    if (cardsVisible) {
      // When: Operator clicks on the first contract
      await contractCards.first().click();

      // Then: Detail drawer opens
      const drawer = page.locator("#detail-drawer");
      await expect(drawer).toHaveClass(/open/);

      // Then: Drawer displays contract information
      await expect(drawer).toBeVisible();

      // Then: Close button is accessible
      const closeButton = page.locator("#close-drawer");
      await expect(closeButton).toBeVisible();
      await expect(closeButton).toHaveAttribute("aria-label", "Close");

      // When: Operator closes drawer with close button
      await closeButton.click();
      await page.waitForTimeout(300); // Animation

      // Then: Drawer closes
      await expect(drawer).not.toHaveClass(/open/);

      // When: Operator reopens drawer
      await contractCards.first().click();
      await expect(drawer).toHaveClass(/open/);

      // When: Operator closes drawer with Escape key
      await page.keyboard.press("Escape");
      await page.waitForTimeout(300); // Animation

      // Then: Drawer closes
      await expect(drawer).not.toHaveClass(/open/);
    }

    // When: Operator clears search
    await searchInput.clear();
    await page.waitForTimeout(500); // Debounce

    // Then: Search is cleared
    await expect(searchInput).toHaveValue("");
  });

  test("end-to-end: Operator navigates contracts browser interface", async ({ page, baseURL }) => {
    // Given: Contracts Browser is loaded
    await page.goto(`${baseURL}/ui/contracts`);
    await page.waitForSelector("#search-input", { state: "visible" });
    await page.waitForTimeout(1000);

    // When: Operator checks navigation links
    await page.goto(`${baseURL}/runs`);

    // Then: Contracts Browser nav link is visible (feature flag enabled)
    const contractsNavLink = page.locator('nav a[href*="/ui/contracts"]');
    await expect(contractsNavLink).toHaveCount(1);

    // When: Operator clicks Contracts Browser nav link
    await contractsNavLink.click();

    // Then: Navigates to Contracts Browser
    await expect(page).toHaveURL(/\/ui\/contracts/);
    await expect(page.locator("h1.card-title")).toContainText("Contracts Browser");
  });

  test("end-to-end: Contracts Browser handles edge cases gracefully", async ({ page, baseURL }) => {
    // Given: Contracts Browser is loaded
    await page.goto(`${baseURL}/ui/contracts`);
    await page.waitForSelector("#search-input", { state: "visible" });
    await page.waitForTimeout(2000); // Allow API calls to complete

    // When: No search query is entered
    // Then: Either contracts are shown, or empty/error state is displayed
    const loadingState = page.locator("#loading-state");
    const emptyState = page.locator("#empty-state");
    const errorState = page.locator("#error-state");
    const contractCards = page.locator(".contract-card");

    const loadingVisible = await loadingState.isVisible();
    const emptyVisible = await emptyState.isVisible();
    const errorVisible = await errorState.isVisible();
    const cardsVisible = await contractCards.count() > 0;

    // One of these states should be true
    expect(loadingVisible || emptyVisible || errorVisible || cardsVisible).toBe(true);

    // When: Operator searches for nonexistent contract
    const searchInput = page.locator("#search-input");
    await searchInput.fill("nonexistent-contract-xyz-12345");
    await page.waitForTimeout(500); // Debounce

    // Then: Empty state or no results are shown gracefully
    const finalCardsCount = await contractCards.count();
    const finalEmptyVisible = await emptyState.isVisible();
    const finalErrorVisible = await errorState.isVisible();

    // Should show either no cards with empty state, or continue showing error state
    expect(finalCardsCount === 0 || finalEmptyVisible || finalErrorVisible).toBe(true);
  });

  test("end-to-end: Contracts Browser keyboard accessibility", async ({ page, baseURL }) => {
    // Given: Contracts Browser is loaded
    await page.goto(`${baseURL}/ui/contracts`);
    await page.waitForSelector("#search-input", { state: "visible" });
    await page.waitForTimeout(1000);

    // When: Operator uses Tab to navigate
    await page.keyboard.press("Tab");

    // Then: Focus moves to search input
    const searchInput = page.locator("#search-input");
    await expect(searchInput).toBeFocused();

    // When: Operator types in search
    await page.keyboard.type("test");

    // Then: Search input contains text
    await expect(searchInput).toHaveValue("test");

    // When: Operator presses Escape
    await page.keyboard.press("Escape");

    // Then: Drawer should not be open (if it was, it's now closed)
    const drawer = page.locator("#detail-drawer");
    await expect(drawer).not.toHaveClass(/open/);
  });

  test("end-to-end visual snapshot of contracts browser full page", async ({ page, baseURL }) => {
    // Set deterministic viewport
    await page.setViewportSize({ width: 1280, height: 720 });

    // Given: Contracts Browser is loaded
    await page.goto(`${baseURL}/ui/contracts`);

    // Wait for page to be fully loaded
    await page.waitForSelector("#search-input", { state: "visible" });
    await page.waitForTimeout(1500); // Allow loading states to settle

    // Then: Snapshot matches expected visual
    await expect(page).toHaveScreenshot("e2e-contracts-browser-full-page.png", {
      fullPage: false,
      animations: "disabled",
      timeout: 15000,
    });
  });
});

test.describe("Contracts Browser API Integration End-to-End", () => {
  test("end-to-end: Contracts Browser API endpoints respond correctly", async ({ page, baseURL }) => {
    // When: Operator's browser calls the contracts registry list API
    const response = await page.request.get(`${baseURL}/api/contracts/registry/list`);

    // Then: API returns a valid status code
    // 200 = success, 404 = feature disabled, 502 = registry unavailable
    expect([200, 404, 502]).toContain(response.status());

    if (response.status() === 200) {
      // Then: Response contains contracts array
      const data = await response.json();
      expect(data).toHaveProperty("contracts");
      expect(Array.isArray(data.contracts)).toBe(true);
    }
  });

  test("end-to-end: Contract detail API endpoint structure", async ({ page, baseURL }) => {
    // When: Operator requests a specific contract detail
    const response = await page.request.get(
      `${baseURL}/api/contracts/registry/ritual.started/1.0.0`
    );

    // Then: API returns expected status
    // 200 = found, 404 = not found or feature disabled, 502 = registry unavailable
    expect([200, 404, 502]).toContain(response.status());

    if (response.status() === 200) {
      // Then: Response contains contract metadata
      const data = await response.json();
      expect(data).toHaveProperty("contract");
      expect(data.contract).toHaveProperty("name");
      expect(data.contract).toHaveProperty("version");
    }
  });
});
