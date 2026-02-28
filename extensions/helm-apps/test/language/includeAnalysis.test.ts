import test from "node:test";
import assert from "node:assert/strict";

import { analyzeIncludes } from "../../src/language/includeAnalysis";

test("detects unresolved include usage", () => {
  const yaml = `apps-stateless:\n  api:\n    _include:\n      - missing-profile\n`;
  const result = analyzeIncludes(yaml, []);
  assert.equal(result.unresolvedUsages.length, 1);
  assert.equal(result.unresolvedUsages[0].name, "missing-profile");
});

test("detects unused local include definitions", () => {
  const yaml = `global:\n  _includes:\n    apps-default:\n      enabled: true\napps-stateless:\n  api:\n    enabled: true\n`;
  const result = analyzeIncludes(yaml, []);
  assert.equal(result.unusedDefinitions.length, 1);
  assert.equal(result.unusedDefinitions[0].name, "apps-default");
});

test("treats file include definitions as resolved for usages", () => {
  const yaml = `apps-stateless:\n  api:\n    _include:\n      - file-profile\n`;
  const result = analyzeIncludes(yaml, [{ name: "file-profile" }]);
  assert.equal(result.unresolvedUsages.length, 0);
});
