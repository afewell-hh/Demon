import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30000,
  retries: process.env.CI ? 2 : 0,

  // Snapshot configuration
  snapshotPathTemplate: "../tests/__artifacts__/snapshots/{testFilePath}/{arg}{ext}",

  // Update snapshots in update mode
  updateSnapshots: process.env.UPDATE_SNAPSHOTS === "true" ? "all" : "missing",

  use: {
    baseURL: process.env.BASE_URL || "http://localhost:3000",
    actionTimeout: 10000,
    navigationTimeout: 15000,

    // Snapshot settings for deterministic screenshots
    screenshot: "only-on-failure",
  },

  // Expect configuration for snapshot matching
  expect: {
    toHaveScreenshot: {
      // Allow small pixel differences for cross-platform consistency
      maxDiffPixels: 100,
      // Use consistent threshold
      threshold: 0.2,
    },
  },
});

