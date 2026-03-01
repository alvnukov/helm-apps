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

test("extract works when cursor is inside nested child block", () => {
  const src = `apps-stateless:\n  api:\n    enabled: true\n    labels: |-\n      app: api\n      team: core\n    containers: |-\n      - name: app\n`;

  const out = extractAppChildToGlobalInclude(src, 4, "apps-labels");
  assert.match(out.updatedText, /global:\n  _includes:\n    apps-labels:/);
  assert.match(out.updatedText, /apps-stateless:\n  api:\n    _include:\n      - apps-labels\n    enabled: true/);
  assert.doesNotMatch(out.updatedText, /\n    labels:/);
});

test("extract nested key keeps include on the same nesting level", () => {
  const src = `apps-stateless:\n  app-1:\n    enabled: true\n    containers:\n      app-1:\n        image:\n          name: nginx\n          staticTag: latest\n        ports: |-\n          - name: http\n            containerPort: 80\n`;

  const out = extractAppChildToGlobalInclude(src, 8, "ports-defaults");
  assert.match(out.updatedText, /global:\n  _includes:\n    ports-defaults:\n      ports: \|-/);
  assert.match(out.updatedText, /containers:\n      app-1:\n        _include:\n          - ports-defaults\n        image:/);
  assert.doesNotMatch(out.updatedText, /containers:\n\s+app-1:\n(?:.*\n){0,8}\s+ports:\s+\|-/);
});

test("extract sequential nested keys into same include merges without data loss", () => {
  const src = `apps-stateless:\n  app-1:\n    enabled: true\n    containers:\n      app-1:\n        image:\n          name: nginx\n          staticTag: latest\n        ports: |-\n          - name: http\n            containerPort: 80\n`;

  const first = extractAppChildToGlobalInclude(src, 8, "container-defaults");
  const secondLine = first.updatedText.split(/\r?\n/).findIndex((line) => line.trim() === "image:");
  assert.ok(secondLine >= 0);
  const second = extractAppChildToGlobalInclude(first.updatedText, secondLine, "container-defaults");

  assert.match(second.updatedText, /container-defaults:\n(?:.*\n)*\s{6}ports: \|-/);
  assert.match(second.updatedText, /container-defaults:\n(?:.*\n)*\s{6}image:/);
  assert.match(second.updatedText, /containers:\n\s+app-1:\n\s+_include:\n\s+- container-defaults/);
  assert.doesNotMatch(second.updatedText, /containers:\n\s+app-1:\n(?:.*\n){0,20}\s+image:/);
  assert.doesNotMatch(second.updatedText, /containers:\n\s+app-1:\n(?:.*\n){0,20}\s+ports:/);
});

test("extract to same include fails when key already exists in include profile", () => {
  const src = `global:\n  _includes:\n    app-common:\n      labels: |-\n        team: platform\napps-stateless:\n  app-1:\n    labels: |-\n      app: app-1\n`;

  assert.throws(
    () => extractAppChildToGlobalInclude(src, 7, "app-common"),
    /already contains key 'labels'/,
  );
});

test("extract app child with existing inline include list keeps items and normalizes to multiline", () => {
  const src = `apps-stateless:\n  app-1:\n    _include: ["base", "common"]\n    labels: |-\n      team: platform\n`;

  const out = extractAppChildToGlobalInclude(src, 3, "labels-default");
  assert.match(out.updatedText, /_include:\n\s+- base\n\s+- common\n\s+- labels-default/);
  assert.match(out.updatedText, /global:\n  _includes:\n    labels-default:\n      labels: \|-/);
});

test("extract nested key with existing scalar include preserves it", () => {
  const src = `apps-stateless:\n  app-1:\n    containers:\n      app-1:\n        _include: base-container\n        image:\n          name: nginx\n          staticTag: latest\n        ports: |-\n          - name: http\n            containerPort: 80\n`;

  const out = extractAppChildToGlobalInclude(src, 8, "ports-default");
  assert.match(out.updatedText, /containers:\n\s+app-1:\n\s+_include:\n\s+- base-container\n\s+- ports-default/);
  assert.match(out.updatedText, /global:\n  _includes:\n    ports-default:\n      ports: \|-/);
});

test("extract to include does not duplicate include name", () => {
  const src = `apps-stateless:\n  app-1:\n    _include:\n      - app-common\n    labels: |-\n      app: app-1\n`;

  const first = extractAppChildToGlobalInclude(src, 4, "app-common");
  const includeCount = (first.updatedText.match(/- app-common/g) ?? []).length;
  assert.equal(includeCount, 1);
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
