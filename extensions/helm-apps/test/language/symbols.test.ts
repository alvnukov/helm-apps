import test from "node:test";
import assert from "node:assert/strict";

import { collectSymbolOccurrences, findSymbolAtPosition } from "../../src/language/symbols";

const SAMPLE = `global:
  _includes:
    apps-default:
      enabled: true
  releases:
    prod:
      api: "1.0.0"
apps-stateless:
  api:
    _include:
      - apps-default
    enabled: true
`;

test("finds include symbol under _include usage", () => {
  const line = 10;
  const char = 10;
  const symbol = findSymbolAtPosition(SAMPLE, line, char);
  assert.deepEqual(symbol, { kind: "include", name: "apps-default" });
});

test("collects include definition and usage occurrences", () => {
  const occurrences = collectSymbolOccurrences(SAMPLE, { kind: "include", name: "apps-default" });
  assert.equal(occurrences.length, 2);
  assert.equal(occurrences.filter((o) => o.role === "definition").length, 1);
  assert.equal(occurrences.filter((o) => o.role === "usage").length, 1);
});

test("finds app symbol from app definition and releases usage", () => {
  const symbolAtApp = findSymbolAtPosition(SAMPLE, 8, 3);
  assert.deepEqual(symbolAtApp, { kind: "app", name: "api" });

  const symbolAtRelease = findSymbolAtPosition(SAMPLE, 6, 7);
  assert.deepEqual(symbolAtRelease, { kind: "app", name: "api" });
});

test("collects app definition and release usage", () => {
  const occurrences = collectSymbolOccurrences(SAMPLE, { kind: "app", name: "api" });
  assert.equal(occurrences.length, 2);
  assert.equal(occurrences.filter((o) => o.role === "definition").length, 1);
  assert.equal(occurrences.filter((o) => o.role === "usage").length, 1);
});
