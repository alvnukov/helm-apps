export interface IncludeDefinitionRef {
  name: string;
  line: number;
  source: "local" | "file";
}

export interface IncludeUsageRef {
  name: string;
  line: number;
}

export interface IncludeAnalysisResult {
  definitions: IncludeDefinitionRef[];
  usages: IncludeUsageRef[];
  unresolvedUsages: IncludeUsageRef[];
  unusedDefinitions: IncludeDefinitionRef[];
}

export function analyzeIncludes(
  text: string,
  fileDefinitions: Array<{ name: string }>,
): IncludeAnalysisResult {
  const localDefinitions = collectLocalIncludeDefinitions(text);
  const fileDefs: IncludeDefinitionRef[] = fileDefinitions.map((d) => ({
    name: d.name,
    line: 0,
    source: "file",
  }));
  const definitions = dedupeDefinitions([...localDefinitions, ...fileDefs]);
  const usages = collectIncludeUsages(text);
  const definedNames = new Set(definitions.map((d) => d.name));
  const unresolvedUsages = usages.filter((u) => !definedNames.has(u.name));
  const usedNames = new Set(usages.map((u) => u.name));
  const unusedDefinitions = localDefinitions.filter((d) => !usedNames.has(d.name));
  return { definitions, usages, unresolvedUsages, unusedDefinitions };
}

function collectLocalIncludeDefinitions(text: string): IncludeDefinitionRef[] {
  const lines = text.split(/\r?\n/);
  const out: IncludeDefinitionRef[] = [];
  let inGlobal = false;
  let inIncludes = false;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const key = parseKeyLine(line);
    if (!key) {
      continue;
    }
    const { indent, name } = key;
    if (indent === 0) {
      inGlobal = name === "global";
      inIncludes = false;
      continue;
    }
    if (inGlobal && indent === 2) {
      inIncludes = name === "_includes";
      continue;
    }
    if (inGlobal && inIncludes && indent === 4) {
      out.push({ name, line: i, source: "local" });
    }
  }

  return out;
}

function collectIncludeUsages(text: string): IncludeUsageRef[] {
  const lines = text.split(/\r?\n/);
  const out: IncludeUsageRef[] = [];

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const key = parseKeyLine(line);
    if (key && key.name === "_include") {
      const inline = key.value.trim();
      if (inline.startsWith("[") && inline.endsWith("]")) {
        const inside = inline.slice(1, -1);
        for (const part of inside.split(",")) {
          const val = unquote(part.trim());
          if (isToken(val)) {
            out.push({ name: val, line: i });
          }
        }
      } else {
        const val = unquote(inline);
        if (isToken(val)) {
          out.push({ name: val, line: i });
        }
      }
      continue;
    }

    const item = parseListItemLine(line);
    if (!item) {
      continue;
    }
    const parent = findParentKey(lines, i, item.indent);
    if (parent?.name === "_include") {
      out.push({ name: item.value, line: i });
    }
  }
  return out;
}

function dedupeDefinitions(items: IncludeDefinitionRef[]): IncludeDefinitionRef[] {
  const seen = new Set<string>();
  const out: IncludeDefinitionRef[] = [];
  for (const item of items) {
    if (seen.has(item.name)) {
      continue;
    }
    seen.add(item.name);
    out.push(item);
  }
  return out;
}

function findParentKey(lines: string[], line: number, indent: number): { name: string; indent: number } | null {
  for (let i = line - 1; i >= 0; i -= 1) {
    const key = parseKeyLine(lines[i] ?? "");
    if (!key) {
      continue;
    }
    if (key.indent < indent) {
      return key;
    }
  }
  return null;
}

function parseKeyLine(line: string): { indent: number; name: string; value: string } | null {
  const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  if (!m) {
    return null;
  }
  return { indent: m[1].length, name: m[2], value: m[3] ?? "" };
}

function parseListItemLine(line: string): { indent: number; value: string } | null {
  const m = line.match(/^(\s*)-\s+([A-Za-z0-9_.-]+)\s*$/);
  if (!m) {
    return null;
  }
  return { indent: m[1].length, value: m[2] };
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length > 1 && ((v.startsWith("\"") && v.endsWith("\"")) || (v.startsWith("'") && v.endsWith("'")))) {
    return v.slice(1, -1);
  }
  return v;
}

function isToken(value: string): boolean {
  return /^[A-Za-z0-9_.-]+$/.test(value);
}
