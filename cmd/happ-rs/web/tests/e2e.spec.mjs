import { test, expect } from "@playwright/test";
import { AxeBuilder } from "@axe-core/playwright";
import { openUtility, stabilizePage } from "./helpers.mjs";

test("main flows and converter behavior work", async ({ page }) => {
  await page.goto("/");
  await stabilizePage(page);

  await expect(page.getByRole("heading", { name: "Main Import" })).toBeVisible();
  await openUtility(page, "Converters");
  await expect(page.getByRole("heading", { name: "Converters" })).toBeVisible();

  await page.locator("select").first().selectOption("text-to-hex");
  const inputEditor = page.locator(".conv-grid .cm-editor").first();
  await inputEditor.click();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("happ");
  await page.getByRole("button", { name: "plain" }).click();

  const output = page.locator(".conv-grid .code-output, .conv-grid .hexdump-view").nth(0);
  await expect(output).toContainText("68617070");
});

test("accessibility smoke has no critical issues", async ({ page }) => {
  await page.goto("/");
  await stabilizePage(page);
  const results = await new AxeBuilder({ page }).analyze();
  const severe = results.violations.filter((v) => v.impact === "critical");
  expect(severe, `Critical axe violations: ${JSON.stringify(severe, null, 2)}`).toEqual([]);
});
