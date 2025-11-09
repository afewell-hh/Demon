import { test, expect } from "@playwright/test";

test.describe("Canvas UI", () => {
  test("loads with feature flag enabled", async ({ page, baseURL }) => {
    // In CI, OPERATE_UI_FLAGS=canvas-ui should be set
    const response = await page.goto(`${baseURL}/canvas`);

    // Should get 200 when feature flag is enabled
    expect(response?.status()).toBe(200);
  });

  test("displays canvas viewer interface", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/canvas`);

    // Check that page title is correct
    await expect(page).toHaveTitle(/Canvas DAG Viewer/);

    // Check for canvas container
    await expect(page.locator(".canvas-container")).toBeVisible();

    // Check for SVG element
    await expect(page.locator("#canvas-svg")).toBeVisible();
  });

  test("has control buttons", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/canvas`);

    // Check for controls
    const controls = page.locator(".canvas-controls");
    await expect(controls).toBeVisible();
  });

  test("has minimap", async ({ page, baseURL }) => {
    await page.goto(`${baseURL}/canvas`);

    // Check for minimap
    const minimap = page.locator(".minimap");
    await expect(minimap).toBeVisible();

    // Check for minimap SVG
    await expect(page.locator("#minimap-svg")).toBeVisible();
  });

  test("nav link appears when feature flag enabled", async ({ page, baseURL }) => {
    // With OPERATE_UI_FLAGS=canvas-ui in CI, the nav link should appear
    await page.goto(`${baseURL}/runs`);
    const canvasLink = page.locator('nav a[href*="/canvas"]');

    // In CI with the env var set, the link should be present
    await expect(canvasLink).toHaveCount(1);
  });

  test("visual snapshot of canvas viewer", async ({ page, baseURL }) => {
    // Set deterministic viewport
    await page.setViewportSize({ width: 1280, height: 720 });

    await page.goto(`${baseURL}/canvas`);

    // Wait for canvas to be visible
    await page.waitForSelector("#canvas-svg", { state: "visible" });

    // Wait for canvas to fully stabilize (canvas may have dynamic elements)
    await page.waitForTimeout(2000);

    // Take snapshot with increased timeout for stability
    await expect(page).toHaveScreenshot("canvas-viewer.png", {
      fullPage: false,
      animations: "disabled",
      timeout: 15000,
    });
  });

  test("visual snapshot of canvas viewer with controls", async ({ page, baseURL }) => {
    // Set deterministic viewport
    await page.setViewportSize({ width: 1280, height: 720 });

    await page.goto(`${baseURL}/canvas`);

    // Wait for canvas to be visible
    await page.waitForSelector("#canvas-svg", { state: "visible" });

    // Wait for controls to be visible
    await page.waitForSelector(".canvas-controls", { state: "visible" });

    // Wait for canvas to fully stabilize (canvas may have dynamic elements)
    await page.waitForTimeout(2000);

    // Take snapshot of the full canvas area including controls
    await expect(page.locator(".canvas-container")).toHaveScreenshot("canvas-with-controls.png", {
      animations: "disabled",
      timeout: 15000,
    });
  });
});
