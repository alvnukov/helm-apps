import assert from "node:assert/strict";
import test from "node:test";

import { buildFieldDocMarkdown, buildFieldDocMarkdownLocalized, findFieldDoc, findKeyPathAtPosition } from "../../src/hover/fieldHover";

test("finds path for nested key at cursor", () => {
  const yaml = `global:
  env: prod
apps-stateless:
  nginx:
    enabled: true
`;
  const path = findKeyPathAtPosition(yaml, 4, 6);
  assert.deepEqual(path, ["apps-stateless", "nginx", "enabled"]);
});

test("finds doc for wildcard app path", () => {
  const doc = findFieldDoc(["apps-stateless", "nginx", "enabled"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Resource Toggle");
});

test("finds doc for custom group __GroupVars__.type", () => {
  const doc = findFieldDoc(["test-app-stateless", "__GroupVars__", "type"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Group Renderer Type");
});

test("markdown contains key sections", () => {
  const doc = findFieldDoc(["global", "env"]);
  assert.ok(doc);
  const md = buildFieldDocMarkdown(["global", "env"], doc!);
  assert.ok(md.includes("**Environment Selector**"));
  assert.ok(md.includes("**Type**:"));
  assert.ok(md.includes("```yaml"));
});

test("returns dynamic doc for built-in top-level group", () => {
  const doc = findFieldDoc(["apps-certificates"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Built-in Group");
});

test("returns dynamic doc for app entry node", () => {
  const doc = findFieldDoc(["apps-stateless", "nginx"]);
  assert.ok(doc);
  assert.equal(doc?.title, "App Entry");
});

test("returns schema-based doc for known key not in manual rules", () => {
  const doc = findFieldDoc(["global", "releases"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Schema Field: releases");
});

test("returns unknown-field doc for custom key under app", () => {
  const doc = findFieldDoc(["apps-stateless", "nginx", "myCustomFlag"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Custom or Unknown Field");
});

test("returns resources doc for kafka-strimzi nested resources key", () => {
  const doc = findFieldDoc(["apps-kafka-strimzi", "test-kafka", "kafka", "resources"]);
  assert.ok(doc);
  assert.equal(doc?.title, "Resources");
});

test("helper markdown includes docs link", () => {
  const doc = findFieldDoc(["apps-stateless", "api", "containers", "main", "configFilesYAML"]);
  assert.ok(doc);
  const md = buildFieldDocMarkdown(["apps-stateless", "api", "containers", "main", "configFilesYAML"], doc!);
  assert.ok(md.includes("https://github.com/alvnukov/helm-apps/blob/main/docs/reference-values.md#param-configfilesyaml"));
});

test("envVars helper points to dedicated envVars docs section", () => {
  const doc = findFieldDoc(["apps-stateful", "app-1", "initContainers", "init-container-1", "envVars"]);
  assert.ok(doc);
  const md = buildFieldDocMarkdown(["apps-stateful", "app-1", "initContainers", "init-container-1", "envVars"], doc!);
  assert.ok(md.includes("https://github.com/alvnukov/helm-apps/blob/main/docs/reference-values.md#param-envvars-usage"));
  assert.ok(md.includes("**Example**:"));
});

test("resources helper has inline example in fallback-by-key mode", () => {
  const doc = findFieldDoc(["apps-kafka-strimzi", "test-kafka", "kafka", "resources"]);
  assert.ok(doc);
  const md = buildFieldDocMarkdown(["apps-kafka-strimzi", "test-kafka", "kafka", "resources"], doc!);
  assert.ok(md.includes("requests:"));
  assert.ok(md.includes("limits:"));
});

test("k8s field markdown switches docs link by locale", () => {
  const doc = findFieldDoc(["apps-stateless", "api", "containers", "main", "command"]);
  assert.ok(doc);
  const ru = buildFieldDocMarkdownLocalized(["apps-stateless", "api", "containers", "main", "command"], doc!, "ru");
  const en = buildFieldDocMarkdownLocalized(["apps-stateless", "api", "containers", "main", "command"], doc!, "en");
  assert.ok(ru.includes("docs/k8s-fields-guide.md#command-and-args"));
  assert.ok(en.includes("docs/k8s-fields-guide.en.md#command-and-args"));
});

test("k8s field markdown contains official kubernetes docs link", () => {
  const doc = findFieldDoc(["apps-stateless", "api", "containers", "main", "livenessProbe"]);
  assert.ok(doc);
  const md = buildFieldDocMarkdown(["apps-stateless", "api", "containers", "main", "livenessProbe"], doc!);
  assert.ok(md.includes("https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/"));
});

test("ignores key-like text inside block scalars", () => {
  const yaml = `apps-stateless:
  nginx:
    labels: |-
      enabled: true
`;
  const path = findKeyPathAtPosition(yaml, 3, 8);
  assert.equal(path, null);
});

test("schema fallback is contextual for initContainers node", () => {
  const path = ["apps-cronjobs", "cronjob-1", "initContainers"];
  const doc = findFieldDoc(path);
  assert.ok(doc);
  assert.equal(doc?.title, "Schema Field: initContainers");
  assert.ok(!(doc?.summary ?? "").includes("defined in values schema"));
});

test("schema fallback for named map entry explains map item semantics", () => {
  const path = ["apps-cronjobs", "cronjob-1", "initContainers", "init-container-1"];
  const doc = findFieldDoc(path);
  assert.ok(doc);
  assert.ok((doc?.summary ?? "").includes("Named entry"));
});
