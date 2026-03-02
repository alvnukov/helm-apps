import { test, expect } from "@playwright/test";
import { openUtility, stabilizePage } from "./helpers.mjs";

test("visual baseline: main import", async ({ page }) => {
  await page.goto("/");
  await stabilizePage(page);
  await expect(page).toHaveScreenshot("main-import.png", {
    fullPage: true,
    maxDiffPixelRatio: 0.02,
  });
});

test("visual baseline: converters", async ({ page }) => {
  await page.goto("/");
  await stabilizePage(page);
  await openUtility(page, "Converters");
  await page.locator("select").first().selectOption("text-to-hex");
  await page.getByRole("button", { name: "plain" }).click();
  await expect(page).toHaveScreenshot("converters-hex.png", {
    fullPage: true,
    maxDiffPixelRatio: 0.02,
  });
});
