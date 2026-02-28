import assert from "node:assert/strict";
import test from "node:test";

import { extractAppChildToGlobalInclude, safeRenameAppKey } from "../../src/refactor/appRefactor";

test("extract app child key to global include", () => {
  const src = `apps-stateless:\n  api:\n    enabled: true\n    labels: |-\n      app: api\n    containers: |-\n      - name: app\n`;

  const out = extractAppChildToGlobalInclude(src, 3, "apps-common");
  assert.match(out.updatedText, /global:\n  _includes:\n    apps-common:/);
  assert.match(out.updatedText, /apps-stateless:\n  api:\n    _include:\n      - apps-common\n    enabled: true/);
  assert.doesNotMatch(out.updatedText, /\n    labels:/);
});

test("safe rename app key and update global releases", () => {
  const src = `global:\n  releases:\n    r1:\n      api: \"1.0.0\"\napps-stateless:\n  api:\n    enabled: true\n`;

  const out = safeRenameAppKey(src, 6, "api-v2");
  assert.match(out.updatedText, /apps-stateless:\n  api-v2:/);
  assert.match(out.updatedText, /global:\n  releases:\n    r1:\n      api-v2: "1.0.0"/);
});

test("safe rename rejects invalid key", () => {
  const src = `apps-stateless:\n  api:\n    enabled: true\n`;
  assert.throws(() => safeRenameAppKey(src, 1, "API V2"));
});

test("extract fails outside app child scope", () => {
  const src = `global:\n  env: dev\n`;
  assert.throws(() => extractAppChildToGlobalInclude(src, 1, "x"));
});
