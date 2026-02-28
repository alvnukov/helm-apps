import assert from "node:assert/strict";
import test from "node:test";

import { findAppScopeAtLine, resolveEntityWithIncludes } from "../../src/preview/includeResolver";

test("finds app scope by cursor line", () => {
  const yaml = `global:\n  env: dev\napps-stateless:\n  api:\n    enabled: true\n    labels: |-\n      app: api\n`;

  const scope = findAppScopeAtLine(yaml, 6);
  assert.deepEqual(scope, { group: "apps-stateless", app: "api" });
});

test("finds app scope inside custom group with __GroupVars__", () => {
  const yaml = `test-app-stateless:\n  __GroupVars__:\n    type: apps-stateless\n  nginx:\n    enabled: true\n`;

  const scope = findAppScopeAtLine(yaml, 4);
  assert.deepEqual(scope, { group: "test-app-stateless", app: "nginx" });
});

test("resolves includes recursively and app overrides profile", () => {
  const values = {
    global: {
      _includes: {
        base: {
          labels: "a: b",
          resources: { limits: "cpu: 100m" },
        },
        team: {
          _include: ["base"],
          annotations: "team: platform",
          resources: { requests: "cpu: 50m" },
        },
      },
    },
    "apps-stateless": {
      api: {
        _include: ["team"],
        enabled: true,
        resources: { limits: "cpu: 200m" },
      },
    },
  };

  const resolved = resolveEntityWithIncludes(values, "apps-stateless", "api");
  assert.equal(resolved.enabled, true);
  assert.equal(resolved.labels, "a: b");
  assert.equal(resolved.annotations, "team: platform");
  assert.deepEqual(resolved.resources, {
    limits: "cpu: 200m",
    requests: "cpu: 50m",
  });
  assert.equal(Object.prototype.hasOwnProperty.call(resolved, "_include"), false);
});

test("throws on include cycles", () => {
  const values = {
    global: {
      _includes: {
        a: { _include: ["b"] },
        b: { _include: ["a"] },
      },
    },
    "apps-services": {
      svc: { _include: ["a"] },
    },
  };

  assert.throws(() => resolveEntityWithIncludes(values, "apps-services", "svc"));
});

test("__GroupVars__ include does not break app resolution", () => {
  const values = {
    global: {
      _includes: {
        common: {
          type: "apps-stateless",
        },
      },
    },
    "apps-stateless": {
      __GroupVars__: {
        _include: ["common"],
      },
      api: {
        enabled: true,
      },
    },
  };

  const resolved = resolveEntityWithIncludes(values, "apps-stateless", "api");
  assert.equal(resolved.enabled, true);
});
