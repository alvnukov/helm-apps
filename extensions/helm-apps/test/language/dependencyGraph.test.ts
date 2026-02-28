import test from "node:test";
import assert from "node:assert/strict";

import { buildDependencyGraphModel } from "../../src/language/dependencyGraph";

test("builds graph model with apps/includes/include files", () => {
  const yaml = `global:
  _includes:
    apps-default:
      enabled: true
apps-stateless:
  api:
    _include:
      - apps-default
    _include_files:
      - defaults.yaml
    enabled: true
`;
  const model = buildDependencyGraphModel(yaml);
  assert.deepEqual(model.includes, ["apps-default"]);
  assert.deepEqual(model.includeFiles, ["defaults.yaml"]);
  assert.equal(model.apps.length, 1);
  assert.equal(model.apps[0].group, "apps-stateless");
  assert.equal(model.apps[0].app, "api");
  assert.deepEqual(model.apps[0].includes, ["apps-default"]);
});
