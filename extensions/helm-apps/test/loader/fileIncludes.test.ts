import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { expandValuesWithFileIncludes } from "../../src/loader/fileIncludes";

test("expands _include_files into global._includes and _include", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const includeFile = join(dir, "apps-common.yaml");
  await writeFile(includeFile, "labels: |-\n  team: platform\n", "utf8");

  const values = {
    global: { _includes: {} },
    "apps-stateless": {
      api: {
        _include_files: ["apps-common.yaml"],
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const app = ((expanded.values["apps-stateless"] as Record<string, unknown>).api as Record<string, unknown>);
  assert.deepEqual(app._include, ["apps-common"]);

  const gl = expanded.values.global as Record<string, unknown>;
  const includes = gl._includes as Record<string, unknown>;
  assert.ok(includes["apps-common"]);
  assert.equal(expanded.includeDefinitions[0].name, "apps-common");
  assert.equal(expanded.includeDefinitions[0].filePath, includeFile);
});

test("expands _include_from_file and lets local override", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const includeFile = join(dir, "base.yaml");
  await writeFile(includeFile, "enabled: true\nlabels: |\n  a: b\n", "utf8");

  const values = {
    "apps-services": {
      svc: {
        _include_from_file: "base.yaml",
        enabled: false,
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const svc = ((expanded.values["apps-services"] as Record<string, unknown>).svc as Record<string, unknown>);
  assert.equal(svc.enabled, false);
  assert.equal(svc.labels, "a: b\n");
  assert.equal(Object.prototype.hasOwnProperty.call(svc, "_include_from_file"), false);
});

test("registers include definitions loaded via global._includes._include_from_file", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const includeFile = join(dir, "helm-apps-defaults.yaml");
  await writeFile(includeFile, "apps-ingresses-defaultIngress:\n  class: nginx\n", "utf8");

  const values = {
    global: {
      _includes: {
        _include_from_file: "helm-apps-defaults.yaml",
      },
    },
    "apps-ingresses": {
      ingress1: {
        _include: ["apps-ingresses-defaultIngress"],
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const gl = expanded.values.global as Record<string, unknown>;
  const includes = gl._includes as Record<string, unknown>;
  assert.ok(includes["apps-ingresses-defaultIngress"]);
  assert.ok(expanded.includeDefinitions.some((d) =>
    d.name === "apps-ingresses-defaultIngress" && d.filePath === includeFile));
});

test("does not resolve include file from parent directories", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const nested = join(dir, "a", "b");
  await mkdir(nested, { recursive: true });

  const includeFile = join(dir, "shared-values.yaml");
  await writeFile(includeFile, "enabled: true\n", "utf8");

  const values = {
    "apps-services": {
      svc: {
        _include_from_file: "shared-values.yaml",
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(nested, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const svc = ((expanded.values["apps-services"] as Record<string, unknown>).svc as Record<string, unknown>);
  assert.equal(svc.enabled, undefined);
  assert.equal(expanded.missingFiles.length, 1);
  assert.equal(expanded.missingFiles[0].rawPath, "shared-values.yaml");
});

test("skips missing include files without error", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));

  const values = {
    "apps-services": {
      svc: {
        _include_from_file: "missing-values.yaml",
        _include_files: ["missing-profile.yaml"],
        enabled: true,
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const svc = ((expanded.values["apps-services"] as Record<string, unknown>).svc as Record<string, unknown>);
  assert.equal(svc.enabled, true);
  assert.equal(Object.prototype.hasOwnProperty.call(svc, "_include_from_file"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(svc, "_include_files"), false);
  assert.deepEqual(svc._include, []);
});

test("skips include path with ENOTDIR without error", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const blocker = join(dir, "config");
  await writeFile(blocker, "not-a-dir", "utf8");

  const values = {
    "apps-services": {
      svc: {
        _include_from_file: "config/test-include-from-file.yaml",
        enabled: true,
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const svc = ((expanded.values["apps-services"] as Record<string, unknown>).svc as Record<string, unknown>);
  assert.equal(svc.enabled, true);
  assert.equal(expanded.missingFiles.length, 1);
  assert.equal(expanded.missingFiles[0].rawPath, "config/test-include-from-file.yaml");
});

test("skips templated include paths without missing-file warnings", async () => {
  const dir = await mkdtemp(join(tmpdir(), "helm-apps-ext-"));
  const values = {
    "apps-services": {
      svc: {
        _include_from_file: "config/test-include-{{ print \"files\" }}.yaml",
      },
    },
  } as Record<string, unknown>;

  const expanded = await expandValuesWithFileIncludes(values, join(dir, "values.yaml"), async (p) =>
    await readFile(p, "utf8"),
  );

  const svc = ((expanded.values["apps-services"] as Record<string, unknown>).svc as Record<string, unknown>);
  assert.equal(Object.prototype.hasOwnProperty.call(svc, "_include_from_file"), false);
  assert.equal(expanded.missingFiles.length, 0);
});
