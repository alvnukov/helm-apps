import assert from "node:assert/strict";
import test from "node:test";

import { buildStarterChartFiles, isValidChartVersion, sanitizeChartName } from "../../src/scaffold/chartScaffold";

test("sanitizeChartName normalizes user input", () => {
  assert.equal(sanitizeChartName(" My App "), "my-app");
  assert.equal(sanitizeChartName("___"), "app");
});

test("isValidChartVersion accepts semver-like values", () => {
  assert.equal(isValidChartVersion("0.1.0"), true);
  assert.equal(isValidChartVersion("1.2.3-beta.1"), true);
  assert.equal(isValidChartVersion("1.0"), false);
});

test("buildStarterChartFiles contains required helm-apps init entrypoint", () => {
  const files = buildStarterChartFiles({ chartName: "demo", chartVersion: "0.1.0", libraryVersion: "1.8.1" });
  assert.ok(files["Chart.yaml"].includes("name: demo"));
  assert.ok(files["Chart.yaml"].includes("dependencies:"));
  assert.ok(files["Chart.yaml"].includes("name: helm-apps"));
  assert.ok(files["Chart.yaml"].includes('version: "1.8.1"'));
  assert.ok(!files["Chart.yaml"].includes("repository:"));
  assert.equal(files["templates/init-helm-apps-library.yaml"].trim(), '{{- include "apps-utils.init-library" $ }}');
});
