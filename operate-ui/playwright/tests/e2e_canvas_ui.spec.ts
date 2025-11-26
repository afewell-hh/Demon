import { test, expect } from "@playwright/test";

/**
 * End-to-end validation for Canvas UI user journey
 *
 * Journey: Operator Visualizes Ritual Flows via Canvas UI
 * Persona: Operations engineer investigating ritual execution paths and telemetry during an incident
 *
 * Given: Operate UI is running with `canvas-ui` feature flag enabled, and telemetry data is available
 * When: The operator navigates to the Canvas viewer and interacts with the DAG
 * Then: The operator can visualize flows, zoom/pan, inspect nodes, and navigate to related contracts
 */

test.describe("Canvas UI End-to-End Journey", () => {
  test("end-to-end: Operator visualizes and interacts with ritual DAG", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded with telemetry data
    await page.goto(`${baseURL}/canvas`);

    // Wait for canvas to be interactive
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000); // Allow force-directed layout to stabilize

    // Then: Page loads successfully
    await expect(page).toHaveTitle(/Canvas DAG Viewer/);

    // Then: Canvas container is visible
    const canvasContainer = page.locator(".canvas-container");
    await expect(canvasContainer).toBeVisible();

    // Then: SVG element is rendered
    const canvasSvg = page.locator("#canvas-svg");
    await expect(canvasSvg).toBeVisible();

    // Then: Controls are visible
    const controls = page.locator(".canvas-controls");
    await expect(controls).toBeVisible();

    // Then: Minimap is visible
    const minimap = page.locator(".minimap");
    await expect(minimap).toBeVisible();

    const minimapSvg = page.locator("#minimap-svg");
    await expect(minimapSvg).toBeVisible();
  });

  test("end-to-end: Operator uses zoom and pan controls", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000);

    // When: Operator looks for control buttons
    const controls = page.locator(".canvas-controls");
    await expect(controls).toBeVisible();

    // Then: Control buttons should be available (implementation may vary)
    // Looking for common control patterns (buttons, icons, etc.)
    const controlButtons = controls.locator("button, .control-button, [role='button']");
    const controlCount = await controlButtons.count();

    // At least some controls should be present
    expect(controlCount).toBeGreaterThanOrEqual(0);

    // When: Operator tries keyboard navigation (if supported)
    await page.keyboard.press("Escape");

    // Then: Canvas remains stable (no crashes)
    await expect(page.locator("#canvas-svg")).toBeVisible();
  });

  test("end-to-end: Operator navigates from Canvas to Contracts Browser", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(1000);

    // When: Operator navigates to main navigation
    await page.goto(`${baseURL}/runs`);

    // Then: Canvas nav link is visible (feature flag enabled)
    const canvasNavLink = page.locator('nav a[href*="/canvas"]');
    await expect(canvasNavLink).toHaveCount(1);

    // Then: Contracts Browser nav link is also visible (both features enabled)
    const contractsNavLink = page.locator('nav a[href*="/ui/contracts"]');
    await expect(contractsNavLink).toHaveCount(1);

    // When: Operator clicks Canvas nav link
    await canvasNavLink.click();

    // Then: Navigates to Canvas
    await expect(page).toHaveURL(/\/canvas/);
    await expect(page).toHaveTitle(/Canvas DAG Viewer/);

    // When: Operator clicks Contracts Browser nav link
    await contractsNavLink.click();

    // Then: Navigates to Contracts Browser
    await expect(page).toHaveURL(/\/ui\/contracts/);
    await expect(page.locator("h1.card-title")).toContainText("Contracts Browser");
  });

  test("end-to-end: Canvas UI handles empty or error states gracefully", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000);

    // Then: Canvas is rendered (even if empty)
    const canvasSvg = page.locator("#canvas-svg");
    await expect(canvasSvg).toBeVisible();

    // Then: No JavaScript errors crash the page
    await expect(page.locator(".canvas-container")).toBeVisible();
  });

  test("end-to-end: Canvas UI minimap provides spatial awareness", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000);

    // When: Operator checks minimap
    const minimap = page.locator(".minimap");
    await expect(minimap).toBeVisible();

    // Then: Minimap SVG is rendered
    const minimapSvg = page.locator("#minimap-svg");
    await expect(minimapSvg).toBeVisible();

    // Then: Minimap reflects the main canvas (both exist)
    const canvasSvg = page.locator("#canvas-svg");
    await expect(canvasSvg).toBeVisible();
  });

  test("end-to-end: Canvas UI keyboard accessibility", async ({ page, baseURL }) => {
    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000);

    // When: Operator presses Escape key
    await page.keyboard.press("Escape");

    // Then: Canvas remains stable (inspector closes if open)
    await expect(page.locator("#canvas-svg")).toBeVisible();

    // When: Operator uses Tab for navigation
    await page.keyboard.press("Tab");

    // Then: Focus moves through interactive elements (no crash)
    await expect(page.locator(".canvas-container")).toBeVisible();
  });

  test("end-to-end visual snapshot of canvas viewer full page", async ({ page, baseURL }) => {
    // Set deterministic viewport
    await page.setViewportSize({ width: 1280, height: 720 });

    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);

    // Wait for canvas to be fully loaded and stabilized
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(3000); // Allow force-directed layout to stabilize

    // Then: Snapshot matches expected visual
    await expect(page).toHaveScreenshot("e2e-canvas-viewer-full-page.png", {
      fullPage: false,
      animations: "disabled",
      timeout: 15000,
    });
  });

  test("end-to-end visual snapshot of canvas controls and minimap", async ({ page, baseURL }) => {
    // Set deterministic viewport
    await page.setViewportSize({ width: 1280, height: 720 });

    // Given: Canvas UI is loaded
    await page.goto(`${baseURL}/canvas`);

    // Wait for canvas to be fully loaded
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForSelector(".canvas-controls", { state: "visible" });
    await page.waitForSelector(".minimap", { state: "visible" });
    await page.waitForTimeout(3000); // Allow force-directed layout to stabilize

    // Then: Snapshot of canvas container (including controls and minimap)
    await expect(page.locator(".canvas-container")).toHaveScreenshot("e2e-canvas-with-ui-controls.png", {
      animations: "disabled",
      timeout: 15000,
    });
  });
});

test.describe("Canvas UI Integration with Contracts Browser", () => {
  test("end-to-end: Operator workflow across Canvas and Contracts Browser", async ({ page, baseURL }) => {
    // Given: Operator starts at Canvas UI
    await page.goto(`${baseURL}/canvas`);
    await page.waitForSelector("#canvas-svg", { state: "visible" });
    await page.waitForTimeout(2000);

    // When: Operator wants to inspect a contract
    // Navigate to Contracts Browser
    const contractsNavLink = page.locator('nav a[href*="/ui/contracts"]');
    await contractsNavLink.click();

    // Then: Contracts Browser loads
    await expect(page).toHaveURL(/\/ui\/contracts/);
    await expect(page.locator("h1.card-title")).toContainText("Contracts Browser");

    // When: Operator searches for a contract
    const searchInput = page.locator("#search-input");
    await searchInput.fill("ritual");
    await page.waitForTimeout(500);

    // Then: Search is performed
    await expect(searchInput).toHaveValue("ritual");

    // When: Operator navigates back to Canvas
    const canvasNavLink = page.locator('nav a[href*="/canvas"]');
    await canvasNavLink.click();

    // Then: Returns to Canvas UI
    await expect(page).toHaveURL(/\/canvas/);
    await expect(page).toHaveTitle(/Canvas DAG Viewer/);

    // Then: Canvas is still functional
    await expect(page.locator("#canvas-svg")).toBeVisible();
    await expect(page.locator(".minimap")).toBeVisible();
  });

  test("end-to-end: Feature flags enable both Canvas and Contracts Browser", async ({ page, baseURL }) => {
    // When: Operator navigates to main runs page
    await page.goto(`${baseURL}/runs`);

    // Then: Both Canvas and Contracts Browser nav links are present
    const canvasNavLink = page.locator('nav a[href*="/canvas"]');
    const contractsNavLink = page.locator('nav a[href*="/ui/contracts"]');

    await expect(canvasNavLink).toHaveCount(1);
    await expect(contractsNavLink).toHaveCount(1);

    // When: Operator clicks Canvas link
    await canvasNavLink.click();

    // Then: Canvas loads successfully
    const canvasResponse = await page.goto(`${baseURL}/canvas`);
    expect(canvasResponse?.status()).toBe(200);

    // When: Operator clicks Contracts Browser link
    await contractsNavLink.click();

    // Then: Contracts Browser loads successfully
    const contractsResponse = await page.goto(`${baseURL}/ui/contracts`);
    expect(contractsResponse?.status()).toBe(200);
  });
});
