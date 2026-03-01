"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const node_test_1 = __importDefault(require("node:test"));
const strict_1 = __importDefault(require("node:assert/strict"));
const node_fs_1 = require("node:fs");
const node_path_1 = require("node:path");
const listPolicy_1 = require("../../src/validator/listPolicy");
function fixture(name) {
    return (0, node_fs_1.readFileSync)((0, node_path_1.join)(__dirname, "..", "fixtures", name), "utf8");
}
(0, node_test_1.default)("allows _include native list", () => {
    const yaml = `\napps-stateless:\n  api:\n    _include:\n      - apps-defaults\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("allows global._includes list", () => {
    const yaml = `\nglobal:\n  _includes:\n    apps-defaults:\n      _include:\n        - a\n      labels:\n        x: y\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("forbids containers native env list", () => {
    const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        env:\n          - name: A\n            value: B\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 1);
    strict_1.default.equal(issues[0].code, "E_UNEXPECTED_LIST");
    strict_1.default.equal(issues[0].path, "Values.apps-stateless.api.containers.main.env");
});
(0, node_test_1.default)("allows envYAML list content", () => {
    const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        envYAML:\n          default: |-\n            - name: A\n              value: B\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("allows configFilesYAML.content list content", () => {
    const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        configFilesYAML:\n          c1:\n            content:\n              default: |-\n                list:\n                  - a\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("allows service-accounts role rules list fields", () => {
    const yaml = `\napps-service-accounts:\n  sa1:\n    roles:\n      role1:\n        rules:\n          r1:\n            verbs:\n              - get\n            resources:\n              - pods\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("allows service-accounts binding subjects list", () => {
    const yaml = `\napps-service-accounts:\n  sa1:\n    roles:\n      role1:\n        binding:\n          subjects:\n            - kind: ServiceAccount\n              name: sa1\n              namespace: default\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("allows sharedEnvConfigMaps/sharedEnvSecrets lists", () => {
    const yaml = `\napps-stateless:\n  api:\n    containers:\n      main:\n        sharedEnvConfigMaps:\n          - cm1\n        sharedEnvSecrets:\n          - sec1\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("forbids service ports list by default", () => {
    const yaml = `\napps-services:\n  svc:\n    ports:\n      - name: http\n        port: 80\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 1);
    strict_1.default.equal(issues[0].path, "Values.apps-services.svc.ports");
});
(0, node_test_1.default)("allows service ports list when built-in list option enabled", () => {
    const yaml = `\napps-services:\n  svc:\n    ports:\n      - name: http\n        port: 80\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml, { allowNativeListsInBuiltInListFields: true });
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("ignores list markers inside YAML block scalar", () => {
    const yaml = `\napps-k8s-manifests:\n  obj:\n    spec: |-\n      containers:\n        - name: a\n          image: b\n`;
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(yaml);
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("fixture: complex valid sample has no list-policy violations", () => {
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(fixture("valid-values.yaml"));
    strict_1.default.equal(issues.length, 0);
});
(0, node_test_1.default)("fixture: invalid sample reports exact violation lines", () => {
    const issues = (0, listPolicy_1.validateUnexpectedNativeLists)(fixture("invalid-values-native-lists.yaml"));
    const got = issues.map((i) => `${i.line}:${i.path}`);
    strict_1.default.deepEqual(got, [
        "10:Values.apps-stateless.api.containers.main.env",
        "22:Values.apps-services.my-service.ports",
    ]);
});
