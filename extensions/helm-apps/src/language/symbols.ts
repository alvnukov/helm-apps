export type SymbolKind = "include" | "app";

export interface SymbolRef {
  kind: SymbolKind;
  name: string;
}

export interface TextRange {
  line: number;
  start: number;
  end: number;
}

export interface SymbolOccurrence extends TextRange {
  role: "definition" | "usage";
}

interface KeyInfo extends TextRange {
  indent: number;
  key: string;
  valueStart: number;
  value: string;
}

export function findSymbolAtPosition(text: string, line: number, character: number): SymbolRef | null {
  const lines = text.split(/\r?\n/);
  const lineText = lines[line] ?? "";
  const token = tokenUnderCursor(lineText, character);
  if (!token) {
    return null;
  }

  const includeAtCursor = findIncludeAtPosition(lines, line, character);
  if (includeAtCursor) {
    return { kind: "include", name: includeAtCursor };
  }

  const keyInfo = parseKeyLine(lines[line] ?? "", line);
  if (!keyInfo) {
    return null;
  }
  if (character < keyInfo.start || character > keyInfo.end) {
    return null;
  }

  if (isIncludeDefinitionLine(lines, line, keyInfo)) {
    return { kind: "include", name: keyInfo.key };
  }

  if (isAppDefinitionLine(lines, line, keyInfo) || isReleaseAppReferenceLine(lines, line, keyInfo)) {
    return { kind: "app", name: keyInfo.key };
  }

  return null;
}

export function collectSymbolOccurrences(text: string, symbol: SymbolRef): SymbolOccurrence[] {
  const lines = text.split(/\r?\n/);
  if (symbol.kind === "include") {
    return collectIncludeOccurrences(lines, symbol.name);
  }
  return collectAppOccurrences(lines, symbol.name);
}

function collectIncludeOccurrences(lines: string[], includeName: string): SymbolOccurrence[] {
  const out: SymbolOccurrence[] = [];

  for (let i = 0; i < lines.length; i += 1) {
    const key = parseKeyLine(lines[i] ?? "", i);
    if (key && key.key === includeName && isIncludeDefinitionLine(lines, i, key)) {
      out.push({ line: i, start: key.start, end: key.end, role: "definition" });
    }

    const includeUses = includeUsagesOnLine(lines, i);
    for (const usage of includeUses) {
      if (usage.value === includeName) {
        out.push({ line: i, start: usage.start, end: usage.end, role: "usage" });
      }
    }
  }

  return dedupeOccurrences(out);
}

function collectAppOccurrences(lines: string[], appKey: string): SymbolOccurrence[] {
  const out: SymbolOccurrence[] = [];

  for (let i = 0; i < lines.length; i += 1) {
    const key = parseKeyLine(lines[i] ?? "", i);
    if (!key || key.key !== appKey) {
      continue;
    }

    if (isAppDefinitionLine(lines, i, key)) {
      out.push({ line: i, start: key.start, end: key.end, role: "definition" });
      continue;
    }
    if (isReleaseAppReferenceLine(lines, i, key)) {
      out.push({ line: i, start: key.start, end: key.end, role: "usage" });
    }
  }

  return dedupeOccurrences(out);
}

function dedupeOccurrences(items: SymbolOccurrence[]): SymbolOccurrence[] {
  const seen = new Set<string>();
  const out: SymbolOccurrence[] = [];
  for (const it of items) {
    const key = `${it.line}:${it.start}:${it.end}:${it.role}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    out.push(it);
  }
  return out;
}

interface IncludeToken {
  value: string;
  start: number;
  end: number;
}

function findIncludeAtPosition(lines: string[], line: number, character: number): string | null {
  const uses = includeUsagesOnLine(lines, line);
  for (const use of uses) {
    if (character >= use.start && character <= use.end) {
      return use.value;
    }
  }
  return null;
}

function includeUsagesOnLine(lines: string[], line: number): IncludeToken[] {
  const text = lines[line] ?? "";
  const key = parseKeyLine(text, line);
  if (key && key.key === "_include") {
    const inline = key.value.trim();
    if (inline.startsWith("[") && inline.endsWith("]")) {
      return parseInlineIncludeTokens(key.value, key.valueStart);
    }
    const scalar = parseScalarIncludeToken(key.value, key.valueStart);
    return scalar ? [scalar] : [];
  }

  const item = parseListItemLine(text);
  if (!item) {
    return [];
  }
  const parent = findParentKey(lines, line, item.indent);
  if (parent?.key !== "_include") {
    return [];
  }
  return [{ value: item.value, start: item.start, end: item.end }];
}

function parseInlineIncludeTokens(value: string, valueStart: number): IncludeToken[] {
  const out: IncludeToken[] = [];
  const re = /[A-Za-z0-9_.-]+/g;
  for (const m of value.matchAll(re)) {
    const token = m[0];
    const start = valueStart + (m.index ?? 0);
    out.push({ value: token, start, end: start + token.length });
  }
  return out;
}

function parseScalarIncludeToken(value: string, valueStart: number): IncludeToken | null {
  const trimmed = value.trim();
  if (trimmed.length === 0) {
    return null;
  }
  const token = unquote(trimmed);
  if (!/^[A-Za-z0-9_.-]+$/.test(token)) {
    return null;
  }
  const rel = value.indexOf(token);
  const start = rel >= 0 ? valueStart + rel : valueStart;
  return { value: token, start, end: start + token.length };
}

function isIncludeDefinitionLine(lines: string[], line: number, key: KeyInfo): boolean {
  if (key.indent !== 4) {
    return false;
  }
  const parent = findParentKey(lines, line, key.indent);
  if (!parent || parent.indent !== 2 || parent.key !== "_includes") {
    return false;
  }
  const top = findParentKey(lines, parent.line, parent.indent);
  return top?.indent === 0 && top.key === "global";
}

function isAppDefinitionLine(lines: string[], line: number, key: KeyInfo): boolean {
  if (key.indent !== 2 || key.key === "__GroupVars__") {
    return false;
  }
  const top = findParentKey(lines, line, key.indent);
  if (!top || top.indent !== 0) {
    return false;
  }
  if (top.key === "global") {
    return false;
  }
  return true;
}

function isReleaseAppReferenceLine(lines: string[], line: number, key: KeyInfo): boolean {
  if (key.indent !== 6) {
    return false;
  }
  const release = findParentKey(lines, line, key.indent);
  if (!release || release.indent !== 4) {
    return false;
  }
  const releases = findParentKey(lines, release.line, release.indent);
  if (!releases || releases.indent !== 2 || releases.key !== "releases") {
    return false;
  }
  const global = findParentKey(lines, releases.line, releases.indent);
  return global?.indent === 0 && global.key === "global";
}

interface ParentKey {
  line: number;
  indent: number;
  key: string;
}

function findParentKey(lines: string[], line: number, currentIndent: number): ParentKey | null {
  for (let i = line - 1; i >= 0; i -= 1) {
    const key = parseKeyLine(lines[i] ?? "", i);
    if (!key) {
      continue;
    }
    if (key.indent < currentIndent) {
      return { line: i, indent: key.indent, key: key.key };
    }
  }
  return null;
}

function parseKeyLine(lineText: string, line: number): KeyInfo | null {
  const m = lineText.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  if (!m) {
    return null;
  }
  const indent = m[1].length;
  const key = m[2];
  const start = lineText.indexOf(key);
  if (start < 0) {
    return null;
  }
  return {
    line,
    indent,
    key,
    start,
    end: start + key.length,
    valueStart: start + key.length + 1,
    value: m[3] ?? "",
  };
}

interface ListItemInfo {
  value: string;
  indent: number;
  start: number;
  end: number;
}

function parseListItemLine(lineText: string): ListItemInfo | null {
  const m = lineText.match(/^(\s*)-\s+([A-Za-z0-9_.-]+)\s*$/);
  if (!m) {
    return null;
  }
  const value = m[2];
  const start = lineText.indexOf(value);
  if (start < 0) {
    return null;
  }
  return {
    value,
    indent: m[1].length,
    start,
    end: start + value.length,
  };
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length > 1 && ((v.startsWith("\"") && v.endsWith("\"")) || (v.startsWith("'") && v.endsWith("'")))) {
    return v.slice(1, -1);
  }
  return v;
}

function tokenUnderCursor(line: string, char: number): string | null {
  const re = /[A-Za-z0-9_.-]+/g;
  for (const m of line.matchAll(re)) {
    const start = m.index ?? 0;
    const end = start + m[0].length;
    if (char >= start && char <= end) {
      return m[0];
    }
  }
  return null;
}
