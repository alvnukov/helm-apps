import test from "node:test";
import assert from "node:assert/strict";

import { isAllowedListPath } from "../../src/validator/listPolicy";

test("allows explicit policy exception paths", () => {
  const allowed: Array<{ path: string; key: string }> = [
    { path: "Values.apps-stateless.app._include", key: "_include" },
    { path: "Values.apps-stateless.app._include_files", key: "_include_files" },
    { path: "Values.global._includes.apps-defaults._include", key: "_include" },
    { path: "Values.apps-kafka-strimzi.kafka.kafka.brokers.hosts.default", key: "default" },
    { path: "Values.apps-kafka-strimzi.kafka.kafka.ui.dex.allowedGroups.default", key: "default" },
    { path: "Values.apps-stateless.api.containers.main.configFilesYAML.c1.content.default", key: "default" },
    { path: "Values.apps-stateless.api.containers.main.envYAML.default", key: "default" },
    { path: "Values.apps-k8s-manifests.obj.extraFields", key: "extraFields" },
    {
      path: "Values.apps-service-accounts.sa.roles.r1.rules.main.verbs",
      key: "verbs",
    },
    {
      path: "Values.apps-service-accounts.sa.clusterRoles.r1.binding.subjects",
      key: "subjects",
    },
    {
      path: "Values.apps-stateless.api.containers.main.sharedEnvConfigMaps",
      key: "sharedEnvConfigMaps",
    },
    {
      path: "Values.apps-stateless.api.initContainers.init.sharedEnvSecrets",
      key: "sharedEnvSecrets",
    },
  ];

  for (const item of allowed) {
    assert.equal(isAllowedListPath(item.path, item.key, false), true, item.path);
  }
});

test("forbids unknown list paths", () => {
  const forbidden: Array<{ path: string; key: string }> = [
    { path: "Values.apps-stateless.api.containers.main.env", key: "env" },
    { path: "Values.apps-stateful.db.containers.main.ports", key: "ports" },
    { path: "Values.apps-services.api.selector", key: "selector" },
    { path: "Values.apps-jobs.migrate.command", key: "command" },
  ];

  for (const item of forbidden) {
    assert.equal(isAllowedListPath(item.path, item.key, false), false, item.path);
  }
});

test("allows built-in list fields only with feature flag", () => {
  assert.equal(isAllowedListPath("Values.apps-services.svc.ports", "ports", false), false);
  assert.equal(isAllowedListPath("Values.apps-services.svc.ports", "ports", true), true);

  assert.equal(isAllowedListPath("Values.apps-jobs.job.command", "command", false), false);
  assert.equal(isAllowedListPath("Values.apps-jobs.job.command", "command", true), true);
});
