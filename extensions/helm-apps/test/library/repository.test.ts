import test from "node:test";
import assert from "node:assert/strict";

import { compareSemver, normalizeSemverParts, resolveHelmRepositoryURL } from "../../src/library/repository";

test("resolveHelmRepositoryURL maps GitHub repo URL to pages helm repo", () => {
  assert.equal(
    resolveHelmRepositoryURL("https://github.com/alvnukov/helm-apps.git"),
    "https://alvnukov.github.io/helm-apps",
  );
  assert.equal(
    resolveHelmRepositoryURL("git@github.com:alvnukov/helm-apps.git"),
    "https://alvnukov.github.io/helm-apps",
  );
});

test("resolveHelmRepositoryURL keeps explicit helm repo URL", () => {
  assert.equal(
    resolveHelmRepositoryURL("https://alvnukov.github.io/helm-apps/"),
    "https://alvnukov.github.io/helm-apps",
  );
});

test("resolveHelmRepositoryURL rejects invalid input", () => {
  assert.throws(() => resolveHelmRepositoryURL(""), /empty/);
  assert.throws(() => resolveHelmRepositoryURL("alvnukov/helm-apps"), /unsupported/);
});

test("normalizeSemverParts parses common forms", () => {
  assert.deepEqual(normalizeSemverParts("1.8.2"), [1, 8, 2]);
  assert.deepEqual(normalizeSemverParts("v1.8.2"), [1, 8, 2]);
  assert.deepEqual(normalizeSemverParts("1.8.2-rc1"), [1, 8, 2]);
  assert.deepEqual(normalizeSemverParts("1"), [1, 0, 0]);
});

test("compareSemver compares by numeric parts", () => {
  assert.equal(compareSemver("1.8.2", "1.8.2"), 0);
  assert.equal(compareSemver("1.8.3", "1.8.2"), 1);
  assert.equal(compareSemver("1.8.1", "1.8.2"), -1);
  assert.equal(compareSemver("v2.0.0", "1.99.99"), 1);
});
