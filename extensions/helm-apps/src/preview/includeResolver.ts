export interface AppScope {
  group: string;
  app: string;
}

export interface EnvironmentDiscovery {
  literals: string[];
  regexes: string[];
}

export function findAppScopeAtLine(text: string, line: number): AppScope | null {
  const lines = text.split(/\r?\n/);
  const current = Math.min(Math.max(line, 0), Math.max(lines.length - 1, 0));

  for (let i = current; i >= 0; i -= 1) {
    const appKey = parseKey(lines[i]);
    if (!appKey || appKey.indent !== 2 || appKey.name === "__GroupVars__") {
      continue;
    }

    const appEnd = findBlockEnd(lines, i + 1, 2);
    if (current >= appEnd) {
      continue;
    }

    for (let g = i - 1; g >= 0; g -= 1) {
      const groupKey = parseKey(lines[g]);
      if (groupKey && groupKey.indent === 0) {
        if (groupKey.name === "global") {
          break;
        }
        return { group: groupKey.name, app: appKey.name };
      }
    }
  }

  return null;
}

export function resolveEntityWithIncludes(values: unknown, group: string, app: string): Record<string, unknown> {
  const root = toMap(values);
  if (!root) {
    throw new Error("Values document must be a YAML map");
  }

  const expanded = expandIncludesInValues(root);

  const groupMap = toMap(expanded[group]);
  if (!groupMap) {
    throw new Error(`Group not found: ${group}`);
  }

  const appMap = toMap(groupMap[app]);
  if (!appMap) {
    throw new Error(`App not found at ${group}.${app}`);
  }
  return clone(appMap);
}

export function resolveEnvMaps(value: unknown, env: string): unknown {
  if (Array.isArray(value)) {
    return value.map((v) => resolveEnvMaps(v, env));
  }

  if (!isMap(value)) {
    return value;
  }

  if (looksLikeEnvMap(value)) {
    const selected = selectEnvValue(value, env);
    if (selected === value) {
      const out: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(value)) {
        out[k] = resolveEnvMaps(v, env);
      }
      return out;
    }
    return resolveEnvMaps(selected, env);
  }

  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value)) {
    out[k] = resolveEnvMaps(v, env);
  }
  return out;
}

export function discoverEnvironments(values: unknown): EnvironmentDiscovery {
  const literals = new Set<string>();
  const regexes = new Set<string>();

  const root = toMap(values);
  const globalEnv = root && typeof toMap(root.global)?.env === "string" ? String(toMap(root.global)?.env) : "";
  if (globalEnv.trim().length > 0) {
    literals.add(globalEnv.trim());
  }

  walk(values, (map) => {
    if (!looksLikeEnvMap(map)) {
      return;
    }
    for (const key of Object.keys(map)) {
      if (key === "_default") {
        continue;
      }
      if (looksLikeRegexPattern(key)) {
        regexes.add(key);
      } else {
        literals.add(key);
      }
    }
  });

  return {
    literals: [...literals].sort(),
    regexes: [...regexes].sort(),
  };
}

function normalizeInclude(value: unknown): string[] {
  if (typeof value === "string" && value.trim().length > 0) {
    return [value.trim()];
  }
  if (Array.isArray(value)) {
    return value.filter((v): v is string => typeof v === "string" && v.trim().length > 0).map((v) => v.trim());
  }
  return [];
}

function expandIncludesInValues(root: Record<string, unknown>): Record<string, unknown> {
  const includesMap = toMap(toMap(root.global)?._includes) ?? {};
  const profileCache = new Map<string, Record<string, unknown>>();

  const resolveProfile = (name: string, stack: string[]): Record<string, unknown> => {
    if (profileCache.has(name)) {
      return clone(profileCache.get(name) ?? {});
    }
    if (stack.includes(name)) {
      throw new Error(`Include cycle detected: ${[...stack, name].join(" -> ")}`);
    }

    const profile = toMap(includesMap[name]);
    if (!profile) {
      return {};
    }

    let merged: Record<string, unknown> = {};
    for (const child of normalizeInclude(profile._include)) {
      merged = mergeMaps(merged, resolveProfile(child, [...stack, name]));
    }
    merged = mergeMaps(merged, profile);
    delete merged._include;

    profileCache.set(name, merged);
    return clone(merged);
  };

  const expandNode = (node: unknown): unknown => {
    if (Array.isArray(node)) {
      return node.map(expandNode);
    }
    if (!isMap(node)) {
      return node;
    }

    let current = clone(node);
    if (Object.prototype.hasOwnProperty.call(current, "_include")) {
      let merged: Record<string, unknown> = {};
      for (const includeName of normalizeInclude(current._include)) {
        merged = mergeMaps(merged, resolveProfile(includeName, []));
      }
      current = mergeMaps(merged, current);
      delete current._include;
    }

    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(current)) {
      if (k === "_includes") {
        out[k] = cloneValue(v);
      } else {
        out[k] = expandNode(v);
      }
    }
    return out;
  };

  return toMap(expandNode(root)) ?? {};
}

function looksLikeEnvMap(map: Record<string, unknown>): boolean {
  if (Object.prototype.hasOwnProperty.call(map, "_default")) {
    return true;
  }
  for (const key of Object.keys(map)) {
    if (looksLikeRegexPattern(key)) {
      return true;
    }
  }
  return false;
}

function looksLikeRegexPattern(key: string): boolean {
  if (key.length === 0 || key === "_default") {
    return false;
  }
  if (key.startsWith("^") || key.endsWith("$")) {
    return true;
  }
  // treat dot as regex only when used as wildcard pattern (.* / .+ / .?)
  if (key.includes(".*") || key.includes(".+") || key.includes(".?")) {
    return true;
  }
  // char classes, grouping, alternation and escapes are strong regex markers
  return /[\[\]()|\\]/.test(key);
}

function selectEnvValue(map: Record<string, unknown>, env: string): unknown {
  if (Object.prototype.hasOwnProperty.call(map, env)) {
    return map[env];
  }

  for (const [k, v] of Object.entries(map)) {
    if (k === "_default") {
      continue;
    }
    if (!looksLikeRegexPattern(k)) {
      continue;
    }
    try {
      const re = new RegExp(k);
      if (re.test(env)) {
        return v;
      }
    } catch {
      // ignore invalid regex-like patterns
    }
  }

  if (Object.prototype.hasOwnProperty.call(map, "_default")) {
    return map._default;
  }

  return map;
}

function walk(value: unknown, onMap: (m: Record<string, unknown>) => void): void {
  if (Array.isArray(value)) {
    for (const v of value) {
      walk(v, onMap);
    }
    return;
  }
  if (!isMap(value)) {
    return;
  }
  onMap(value);
  for (const v of Object.values(value)) {
    walk(v, onMap);
  }
}

function mergeMaps(base: Record<string, unknown>, incoming: Record<string, unknown>): Record<string, unknown> {
  const out = clone(base);

  for (const [key, value] of Object.entries(incoming)) {
    if (key === "_include") {
      out[key] = [...normalizeInclude(out[key]), ...normalizeInclude(value)];
      continue;
    }

    const current = out[key];
    if (isMap(current) && isMap(value)) {
      out[key] = mergeMaps(current, value);
      continue;
    }

    out[key] = cloneValue(value);
  }

  return out;
}

function parseKey(line: string): { indent: number; name: string } | null {
  const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  if (!m) {
    return null;
  }
  return { indent: m[1].length, name: m[2] };
}

function findBlockEnd(lines: string[], start: number, ownerIndent: number): number {
  for (let i = start; i < lines.length; i += 1) {
    const trimmed = lines[i].trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const indent = countIndent(lines[i]);
    if (indent <= ownerIndent) {
      return i;
    }
  }
  return lines.length;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}

function isMap(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function toMap(value: unknown): Record<string, unknown> | null {
  return isMap(value) ? value : null;
}

function clone<T>(value: T): T {
  return cloneValue(value);
}

function cloneValue<T>(value: T): T {
  if (value === null || value === undefined) {
    return value;
  }
  if (Array.isArray(value) || typeof value === "object") {
    return JSON.parse(JSON.stringify(value)) as T;
  }
  return value;
}
