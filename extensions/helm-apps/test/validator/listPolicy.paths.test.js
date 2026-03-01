"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const node_test_1 = __importDefault(require("node:test"));
const strict_1 = __importDefault(require("node:assert/strict"));
const listPolicy_1 = require("../../src/validator/listPolicy");
(0, node_test_1.default)("allows explicit policy exception paths", () => {
    const allowed = [
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
        strict_1.default.equal((0, listPolicy_1.isAllowedListPath)(item.path, item.key, false), true, item.path);
    }
});
(0, node_test_1.default)("forbids unknown list paths", () => {
    const forbidden = [
        { path: "Values.apps-stateless.api.containers.main.env", key: "env" },
        { path: "Values.apps-stateful.db.containers.main.ports", key: "ports" },
        { path: "Values.apps-services.api.selector", key: "selector" },
        { path: "Values.apps-jobs.migrate.command", key: "command" },
    ];
    for (const item of forbidden) {
        strict_1.default.equal((0, listPolicy_1.isAllowedListPath)(item.path, item.key, false), false, item.path);
    }
});
(0, node_test_1.default)("allows built-in list fields only with feature flag", () => {
    strict_1.default.equal((0, listPolicy_1.isAllowedListPath)("Values.apps-services.svc.ports", "ports", false), false);
    strict_1.default.equal((0, listPolicy_1.isAllowedListPath)("Values.apps-services.svc.ports", "ports", true), true);
    strict_1.default.equal((0, listPolicy_1.isAllowedListPath)("Values.apps-jobs.job.command", "command", false), false);
    strict_1.default.equal((0, listPolicy_1.isAllowedListPath)("Values.apps-jobs.job.command", "command", true), true);
});
