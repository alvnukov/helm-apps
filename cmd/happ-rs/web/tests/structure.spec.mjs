import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";
import { openUtility, stabilizePage } from "./helpers.mjs";

const baselinePath = path.resolve("tests/baseline/ui-structure-baseline.json");
const reportOutPath = path.resolve("test-results/ui-structure-report.json");

function loadBaseline() {
  return JSON.parse(fs.readFileSync(baselinePath, "utf8"));
}

function saveReport(report) {
  fs.mkdirSync(path.dirname(reportOutPath), { recursive: true });
  fs.writeFileSync(reportOutPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
}

async function collectReport(page) {
  return page.evaluate(() => {
    const rect = (el) => {
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return {
        x: Math.round(r.x),
        y: Math.round(r.y),
        width: Math.round(r.width),
        height: Math.round(r.height),
      };
    };
    const textList = (selector) =>
      Array.from(document.querySelectorAll(selector))
        .map((el) => (el.textContent || "").trim())
        .filter(Boolean);
    const queryByText = (tag, text) =>
      Array.from(document.querySelectorAll(tag)).find(
        (el) => (el.textContent || "").trim() === text,
      ) || null;
    const queryByTextContains = (tag, text) =>
      Array.from(document.querySelectorAll(tag)).find((el) =>
        (el.textContent || "").trim().includes(text),
      ) || null;

    const mainImportHeading = queryByText("h3", "Main Import");
    const sourceHeading = queryByTextContains("div", "Source chart values.yaml");
    const generatedHeading = queryByTextContains("div", "Generated values.yaml");

    const rawTitle =
      (
        document.querySelector(".brand h1, .brand, .app-head h1, .app-head .brand")?.textContent ||
        ""
      ).trim();
    const title = rawTitle.split(/\s*Fast local toolset/i)[0].trim();
    return {
      title,
      tabs: textList(".tabs button"),
      topToolbarButtons: textList(".toolbar button, .cardhead .cardbtns button"),
      sections: {
        mainImport: rect(mainImportHeading?.closest(".card") || mainImportHeading),
        sourceValues: rect(sourceHeading?.closest(".section") || sourceHeading),
        generatedValues: rect(generatedHeading?.closest(".section") || generatedHeading),
      },
      hasCodeMirror: !!document.querySelector(".cm-editor"),
    };
  });
}

test("UI structure report matches baseline", async ({ page }) => {
  const baseline = loadBaseline();
  await page.goto("/");
  await expect(page.getByText("happ web")).toBeVisible();
  await stabilizePage(page);
  const mainReport = await collectReport(page);

  await openUtility(page, "Converters");
  const convertersHeading = page.getByRole("heading", { name: "Converters" });
  await expect(convertersHeading).toBeVisible();
  const convertersReport = await page.evaluate(() => {
    const heading = Array.from(document.querySelectorAll("h3")).find(
      (el) => (el.textContent || "").trim() === "Converters",
    );
    const rect = heading
      ? (() => {
          const r = heading.getBoundingClientRect();
          return {
            x: Math.round(r.x),
            y: Math.round(r.y),
            width: Math.round(r.width),
            height: Math.round(r.height),
          };
        })()
      : null;
    return {
      heading: (heading?.textContent || "").trim(),
      hasInputLabel: Array.from(document.querySelectorAll(".panel-label")).some(
        (el) => (el.textContent || "").trim() === "Input",
      ),
      hasOutputLabel: Array.from(document.querySelectorAll(".panel-label")).some(
        (el) => (el.textContent || "").trim() === "Output",
      ),
      rect,
    };
  });

  const report = {
    main: mainReport,
    converters: convertersReport,
  };
  saveReport(report);

  expect(report.main.title).toBe(baseline.main.title);
  expect(report.main.tabs).toEqual(baseline.main.tabs);
  expect(report.main.hasCodeMirror).toBe(true);

  expect(report.main.sections.mainImport?.width || 0).toBeGreaterThanOrEqual(
    baseline.main.minWidths.mainImport,
  );
  expect(report.main.sections.sourceValues?.width || 0).toBeGreaterThanOrEqual(
    baseline.main.minWidths.sourceValues,
  );
  expect(report.main.sections.generatedValues?.width || 0).toBeGreaterThanOrEqual(
    baseline.main.minWidths.generatedValues,
  );

  expect(report.converters.heading).toBe(baseline.converters.heading);
  expect(report.converters.hasInputLabel).toBe(true);
  expect(report.converters.hasOutputLabel).toBe(true);
  expect(report.converters.rect?.width || 0).toBeGreaterThanOrEqual(
    baseline.converters.minHeadingWidth,
  );
});
