import assert from "node:assert/strict";
import test from "node:test";

import { extractLocalIncludeBlock, trimPreview } from "../../src/hover/includeHover";

test("extract local include block", () => {
  const text = `global:\n  _includes:\n    common:\n      labels: |-\n        team: platform\n    another:\n      enabled: true\napps-stateless:\n  api:\n    enabled: true\n`;

  const block = extractLocalIncludeBlock(text, "common");
  assert.equal(block, "common:\n  labels: |-\n    team: platform");
});

test("trim preview by lines and chars", () => {
  const many = Array.from({ length: 120 }, (_, i) => `k${i}: v`).join("\n");
  const trimmed = trimPreview(many, 10, 500);
  assert.match(trimmed, /truncated/);
});
