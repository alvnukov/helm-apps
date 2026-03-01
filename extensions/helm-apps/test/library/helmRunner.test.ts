import test from "node:test";
import assert from "node:assert/strict";

import { buildHelmCommandCandidates } from "../../src/library/helmRunner";

test("buildHelmCommandCandidates uses helm first by default", () => {
  const got = buildHelmCommandCandidates("helm", ["show", "chart"]);
  assert.deepEqual(got, [
    { cmd: "helm", args: ["show", "chart"] },
    { cmd: "werf", args: ["helm", "show", "chart"] },
  ]);
});

test("buildHelmCommandCandidates uses configured custom binary first", () => {
  const got = buildHelmCommandCandidates("/opt/bin/helm3", ["dependency", "update", "."]);
  assert.deepEqual(got, [
    { cmd: "/opt/bin/helm3", args: ["dependency", "update", "."] },
    { cmd: "werf", args: ["helm", "dependency", "update", "."] },
  ]);
});

test("buildHelmCommandCandidates wraps werf path with helm subcommand", () => {
  const got = buildHelmCommandCandidates("/usr/local/bin/werf", ["pull", "helm-apps"]);
  assert.deepEqual(got, [
    { cmd: "/usr/local/bin/werf", args: ["helm", "pull", "helm-apps"] },
    { cmd: "werf", args: ["helm", "pull", "helm-apps"] },
  ]);
});

test("buildHelmCommandCandidates deduplicates equal candidates", () => {
  const got = buildHelmCommandCandidates("werf", ["show", "chart", "helm-apps"]);
  assert.deepEqual(got, [
    { cmd: "werf", args: ["helm", "show", "chart", "helm-apps"] },
  ]);
});
