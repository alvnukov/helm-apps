import assert from "node:assert/strict";
import test from "node:test";

import { discoverEnvironments, resolveEnvMaps } from "../../src/preview/includeResolver";

test("discover environments including regex patterns", () => {
  const values = {
    global: { env: "dev" },
    "apps-stateless": {
      api: {
        labels: {
          _default: { tier: "backend" },
          prod: { tier: "backend-prod" },
          "^staging-.*$": { tier: "backend-stg" },
        },
      },
    },
  };

  const found = discoverEnvironments(values);
  assert.deepEqual(found.literals, ["dev", "prod"]);
  assert.deepEqual(found.regexes, ["^staging-.*$"]);
});

test("resolve env maps by exact key, then regex, then _default", () => {
  const source = {
    labels: {
      _default: { env: "default" },
      prod: { env: "prod" },
      "^staging-.*$": { env: "staging" },
    },
  };

  assert.deepEqual(resolveEnvMaps(source, "prod"), { labels: { env: "prod" } });
  assert.deepEqual(resolveEnvMaps(source, "staging-eu"), { labels: { env: "staging" } });
  assert.deepEqual(resolveEnvMaps(source, "dev"), { labels: { env: "default" } });
});

test("resolve env map without match/default does not recurse infinitely", () => {
  const source = {
    labels: {
      prod: { env: "prod" },
      stage: { env: "stage" },
    },
  };

  const resolved = resolveEnvMaps(source, "dev");
  assert.deepEqual(resolved, {
    labels: {
      prod: { env: "prod" },
      stage: { env: "stage" },
    },
  });
});

test("discover environments does not treat regular dotted keys as regex envs", () => {
  const values = {
    global: { env: "dev" },
    "apps-stateless": {
      nginx: {
        configFilesYAML: {
          "default.conf": "server {}",
          "nginx.conf": "worker_processes auto;",
        },
        labels: {
          _default: { app: "nginx" },
          prod: { app: "nginx-prod" },
        },
      },
    },
  };

  const found = discoverEnvironments(values);
  assert.deepEqual(found.literals, ["dev", "prod"]);
  assert.deepEqual(found.regexes, []);
});
