import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "@playwright/test";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rustRoot = path.resolve(__dirname, "..");

export default defineConfig({
  testDir: "./tests",
  timeout: 45_000,
  expect: {
    timeout: 10_000,
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  outputDir: "test-artifacts",
  reporter: [
    ["list"],
    ["html", { open: "never", outputFolder: "test-results/playwright-report-html" }],
    ["json", { outputFile: "test-results/playwright-report.json" }],
  ],
  use: {
    baseURL: "http://127.0.0.1:18088",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    viewport: { width: 1720, height: 1080 },
  },
  webServer: {
    command: "./target/debug/happ --web --web-addr 127.0.0.1:18088 --web-open-browser=false",
    cwd: rustRoot,
    url: "http://127.0.0.1:18088",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
