import * as YAML from "yaml";

interface AppNode {
  group: string;
  app: string;
  includes: string[];
}

export interface DependencyGraphModel {
  apps: AppNode[];
  includes: string[];
  includeFiles: string[];
}

export function buildDependencyGraphModel(valuesText: string): DependencyGraphModel {
  const parsed = YAML.parse(valuesText) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { apps: [], includes: [], includeFiles: [] };
  }
  const root = parsed as Record<string, unknown>;

  const includes = collectGlobalIncludes(root);
  const includeFiles = collectIncludeFiles(valuesText);
  const apps: AppNode[] = [];

  for (const [group, raw] of Object.entries(root)) {
    if (!isRenderableGroup(group, raw)) {
      continue;
    }
    const groupMap = raw as Record<string, unknown>;
    for (const [app, appRaw] of Object.entries(groupMap)) {
      if (app === "__GroupVars__" || !appRaw || typeof appRaw !== "object" || Array.isArray(appRaw)) {
        continue;
      }
      const appMap = appRaw as Record<string, unknown>;
      apps.push({
        group,
        app,
        includes: normalizeInclude(appMap._include),
      });
    }
  }

  return { apps, includes, includeFiles };
}

function collectGlobalIncludes(root: Record<string, unknown>): string[] {
  const global = root.global;
  if (!global || typeof global !== "object" || Array.isArray(global)) {
    return [];
  }
  const includes = (global as Record<string, unknown>)._includes;
  if (!includes || typeof includes !== "object" || Array.isArray(includes)) {
    return [];
  }
  return Object.keys(includes as Record<string, unknown>).sort();
}

function isRenderableGroup(group: string, raw: unknown): boolean {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return false;
  }
  if (group.startsWith("apps-")) {
    return true;
  }
  return Object.prototype.hasOwnProperty.call(raw as Record<string, unknown>, "__GroupVars__");
}

function normalizeInclude(value: unknown): string[] {
  if (typeof value === "string") {
    return [value];
  }
  if (Array.isArray(value)) {
    return value.filter((v): v is string => typeof v === "string");
  }
  return [];
}

function collectIncludeFiles(text: string): string[] {
  const lines = text.split(/\r?\n/);
  const out = new Set<string>();
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const keyMatch = line.match(/^(\s*)(_include_from_file|_include_files):\s*(.*)$/);
    if (!keyMatch) {
      continue;
    }
    const indent = keyMatch[1].length;
    const key = keyMatch[2];
    const tail = keyMatch[3].trim();

    if (key === "_include_from_file") {
      const value = unquote(tail);
      if (value.length > 0) {
        out.add(value);
      }
      continue;
    }

    if (tail.startsWith("[") && tail.endsWith("]")) {
      for (const part of tail.slice(1, -1).split(",")) {
        const value = unquote(part.trim());
        if (value.length > 0) {
          out.add(value);
        }
      }
      continue;
    }

    for (let j = i + 1; j < lines.length; j += 1) {
      const sub = lines[j];
      const trimmed = sub.trim();
      if (trimmed.length === 0 || trimmed.startsWith("#")) {
        continue;
      }
      const subIndent = countIndent(sub);
      if (subIndent <= indent) {
        break;
      }
      const item = sub.match(/^\s*-\s+(.+)\s*$/);
      if (!item) {
        continue;
      }
      const value = unquote(item[1].trim());
      if (value.length > 0) {
        out.add(value);
      }
    }
  }
  return [...out].sort();
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length > 1 && ((v.startsWith("\"") && v.endsWith("\"")) || (v.startsWith("'") && v.endsWith("'")))) {
    return v.slice(1, -1);
  }
  return v;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}
