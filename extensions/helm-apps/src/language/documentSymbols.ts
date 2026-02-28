import * as vscode from "vscode";

export function buildHelmAppsDocumentSymbols(document: vscode.TextDocument): vscode.DocumentSymbol[] {
  const lines = document.getText().split(/\r?\n/);
  const topSymbols: vscode.DocumentSymbol[] = [];

  for (let i = 0; i < lines.length; i += 1) {
    const top = parseKeyLine(lines[i], i);
    if (!top || top.indent !== 0) {
      continue;
    }

    const topEnd = findBlockEnd(lines, i + 1, 0);
    const topRange = toRange(i, top.start, topEnd - 1, lines[topEnd - 1]?.length ?? top.end);
    const topSymbol = new vscode.DocumentSymbol(top.key, "helm-apps section", vscode.SymbolKind.Namespace, topRange, topRange);

    if (top.key === "global") {
      topSymbol.children.push(...buildGlobalChildren(lines, i, topEnd));
    } else {
      topSymbol.children.push(...buildGroupChildren(lines, i, topEnd));
    }

    topSymbols.push(topSymbol);
  }

  return topSymbols;
}

function buildGlobalChildren(lines: string[], from: number, to: number): vscode.DocumentSymbol[] {
  const out: vscode.DocumentSymbol[] = [];
  for (let i = from + 1; i < to; i += 1) {
    const key = parseKeyLine(lines[i], i);
    if (!key || key.indent !== 2) {
      continue;
    }
    const end = findBlockEnd(lines, i + 1, 2);
    const range = toRange(i, key.start, end - 1, lines[end - 1]?.length ?? key.end);
    const symbol = new vscode.DocumentSymbol(key.key, "global key", vscode.SymbolKind.Property, range, range);
    if (key.key === "_includes") {
      symbol.children.push(...buildIncludeChildren(lines, i, end));
    }
    out.push(symbol);
  }
  return out;
}

function buildIncludeChildren(lines: string[], from: number, to: number): vscode.DocumentSymbol[] {
  const out: vscode.DocumentSymbol[] = [];
  for (let i = from + 1; i < to; i += 1) {
    const key = parseKeyLine(lines[i], i);
    if (!key || key.indent !== 4) {
      continue;
    }
    const end = findBlockEnd(lines, i + 1, 4);
    const range = toRange(i, key.start, end - 1, lines[end - 1]?.length ?? key.end);
    out.push(new vscode.DocumentSymbol(key.key, "include profile", vscode.SymbolKind.Object, range, range));
  }
  return out;
}

function buildGroupChildren(lines: string[], from: number, to: number): vscode.DocumentSymbol[] {
  const out: vscode.DocumentSymbol[] = [];
  for (let i = from + 1; i < to; i += 1) {
    const key = parseKeyLine(lines[i], i);
    if (!key || key.indent !== 2) {
      continue;
    }
    const end = findBlockEnd(lines, i + 1, 2);
    const range = toRange(i, key.start, end - 1, lines[end - 1]?.length ?? key.end);
    const kind = key.key === "__GroupVars__" ? vscode.SymbolKind.Struct : vscode.SymbolKind.Class;
    const detail = key.key === "__GroupVars__" ? "group vars" : "app";
    const symbol = new vscode.DocumentSymbol(key.key, detail, kind, range, range);
    if (key.key !== "__GroupVars__") {
      symbol.children.push(...buildAppFieldChildren(lines, i, end));
    }
    out.push(symbol);
  }
  return out;
}

function buildAppFieldChildren(lines: string[], from: number, to: number): vscode.DocumentSymbol[] {
  const out: vscode.DocumentSymbol[] = [];
  for (let i = from + 1; i < to; i += 1) {
    const key = parseKeyLine(lines[i], i);
    if (!key || key.indent !== 4) {
      continue;
    }
    const end = findBlockEnd(lines, i + 1, 4);
    const range = toRange(i, key.start, end - 1, lines[end - 1]?.length ?? key.end);
    out.push(new vscode.DocumentSymbol(key.key, "field", vscode.SymbolKind.Field, range, range));
  }
  return out;
}

function parseKeyLine(line: string, lineNo: number): { line: number; key: string; indent: number; start: number; end: number } | null {
  const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  if (!m) {
    return null;
  }
  const key = m[2];
  const start = line.indexOf(key);
  if (start < 0) {
    return null;
  }
  return { line: lineNo, key, indent: m[1].length, start, end: start + key.length };
}

function findBlockEnd(lines: string[], start: number, indent: number): number {
  for (let i = start; i < lines.length; i += 1) {
    const t = lines[i].trim();
    if (t.length === 0 || t.startsWith("#")) {
      continue;
    }
    const key = parseKeyLine(lines[i], i);
    if (!key) {
      continue;
    }
    if (key.indent <= indent) {
      return i;
    }
  }
  return lines.length;
}

function toRange(startLine: number, startChar: number, endLine: number, endChar: number): vscode.Range {
  return new vscode.Range(
    new vscode.Position(startLine, Math.max(0, startChar)),
    new vscode.Position(Math.max(startLine, endLine), Math.max(0, endChar)),
  );
}
