import test from "node:test";
import assert from "node:assert/strict";

import { parseYamlKeyTree } from "../../src/structure/treeParser";

test("parses nested YAML key structure", () => {
  const yaml = `
global:
  env: dev
apps-stateless:
  api:
    enabled: true
    containers:
      main:
        image:
          name: nginx
`;

  const roots = parseYamlKeyTree(yaml);
  assert.deepEqual(roots.map((n) => n.label), ["global", "apps-stateless"]);

  const app = roots[1].children[0];
  assert.equal(app.label, "api");
  assert.equal(app.path, "apps-stateless.api");

  const image = app.children.find((n) => n.label === "containers")
    ?.children.find((n) => n.label === "main")
    ?.children.find((n) => n.label === "image");

  assert.ok(image);
  assert.equal(image?.path, "apps-stateless.api.containers.main.image");
});

test("ignores keys inside block scalar", () => {
  const yaml = `
apps-k8s-manifests:
  obj:
    spec: |-
      metadata:
        name: demo
      spec:
        containers:
          - name: app
`;

  const roots = parseYamlKeyTree(yaml);
  const obj = roots[0].children[0];
  const labels = obj.children.map((n) => n.label);
  assert.deepEqual(labels, ["spec"]);
});
