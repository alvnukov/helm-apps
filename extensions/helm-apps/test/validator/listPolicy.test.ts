import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { validateUnexpectedNativeLists } from "../../src/validator/listPolicy";

function fixture(name: string): string {
  return readFileSync(join(process.cwd(), "test", "fixtures", name), "utf8");
}

test("allows _include native list", () => {
  const yaml = `\napps-stateless:\n  api:\n    _include:\n      - apps-defaults\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("allows global._includes list", () => {
  const yaml = `\nglobal:\n  _includes:\n    apps-defaults:\n      _include:\n        - a\n      labels:\n        x: y\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("forbids containers native env list", () => {
  const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        env:\n          - name: A\n            value: B\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 1);
  assert.equal(issues[0].code, "E_UNEXPECTED_LIST");
  assert.equal(issues[0].path, "Values.apps-stateless.api.containers.main.env");
});

test("allows envYAML list content", () => {
  const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        envYAML:\n          default: |-\n            - name: A\n              value: B\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("allows configFilesYAML.content list content", () => {
  const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        configFilesYAML:\n          c1:\n            content:\n              default: |-\n                list:\n                  - a\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("allows service-accounts role rules list fields", () => {
  const yaml = `\napps-service-accounts:\n  sa1:\n    roles:\n      role1:\n        rules:\n          r1:\n            verbs:\n              - get\n            resources:\n              - pods\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("allows service-accounts binding subjects list", () => {
  const yaml = `\napps-service-accounts:\n  sa1:\n    roles:\n      role1:\n        binding:\n          subjects:\n            - kind: ServiceAccount\n              name: sa1\n              namespace: default\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("allows sharedEnvConfigMaps/sharedEnvSecrets lists", () => {
  const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        sharedEnvConfigMaps:\n          - cm1\n        sharedEnvSecrets:\n          - sec1\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("forbids service ports list by default", () => {
  const yaml = `\napps-services:\n  svc:\n    ports:\n      - name: http\n        port: 80\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 1);
  assert.equal(issues[0].path, "Values.apps-services.svc.ports");
});

test("allows service ports list when built-in list option enabled", () => {
  const yaml = `\napps-services:\n  svc:\n    ports:\n      - name: http\n        port: 80\n`;
  const issues = validateUnexpectedNativeLists(yaml, { allowNativeListsInBuiltInListFields: true });
  assert.equal(issues.length, 0);
});

test("ignores list markers inside YAML block scalar", () => {
  const yaml = `\napps-k8s-manifests:\n  obj:\n    spec: |-\n      containers:\n        - name: a\n          image: b\n`;
  const issues = validateUnexpectedNativeLists(yaml);
  assert.equal(issues.length, 0);
});

test("fixture: complex valid sample has no list-policy violations", () => {
  const issues = validateUnexpectedNativeLists(fixture("valid-values.yaml"));
  assert.equal(issues.length, 0);
});

test("fixture: invalid sample reports exact violation lines", () => {
  const issues = validateUnexpectedNativeLists(fixture("invalid-values-native-lists.yaml"));
  const got = issues.map((i) => `${i.line}:${i.path}`);
  assert.deepEqual(got, [
    "10:Values.apps-stateless.api.containers.main.env",
    "22:Values.apps-services.my-service.ports",
  ]);
});
