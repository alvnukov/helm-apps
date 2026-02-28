export interface RefactorResult {
  updatedText: string;
  details: string;
}

export function extractAppChildToGlobalInclude(
  text: string,
  cursorLine: number,
  includeName: string,
): RefactorResult {
  const lines = text.split(/\r?\n/);
  const keyLine = findNearestKeyLine(lines, cursorLine);
  if (keyLine < 0) {
    throw new Error("Place cursor on app child key to extract");
  }

  const key = parseKey(lines[keyLine]);
  if (!key || key.indent !== 4) {
    throw new Error("Extraction supports only direct app child keys (indent=4)");
  }
  if (key.name === "_include") {
    throw new Error("Cannot extract _include key");
  }

  const app = findAncestorWithIndent(lines, keyLine, 2);
  const group = findAncestorWithIndent(lines, keyLine, 0);
  if (!app || !group || !group.name.startsWith("apps-")) {
    throw new Error("Key must be inside apps-*.<app> scope");
  }

  const blockEnd = findBlockEnd(lines, keyLine + 1, key.indent);
  const extracted = lines.slice(keyLine, blockEnd);

  const withoutBlock = [...lines.slice(0, keyLine), ...lines.slice(blockEnd)];
  const withInclude = upsertAppInclude(withoutBlock, app.line, includeName);
  const withGlobalProfile = upsertGlobalIncludeProfile(withInclude, includeName, extracted, key.indent);

  return {
    updatedText: withGlobalProfile.join("\n"),
    details: `extracted ${group.name}.${app.name}.${key.name} -> global._includes.${includeName}`,
  };
}

export function safeRenameAppKey(text: string, cursorLine: number, newKey: string): RefactorResult {
  if (!/^[a-z0-9][a-z0-9.-]*$/.test(newKey)) {
    throw new Error("New app key must match ^[a-z0-9][a-z0-9.-]*$");
  }

  const lines = text.split(/\r?\n/);
  const keyLine = findNearestKeyLine(lines, cursorLine);
  if (keyLine < 0) {
    throw new Error("Place cursor on app key or inside app block");
  }

  const app = findAncestorWithIndent(lines, keyLine, 2);
  const group = findAncestorWithIndent(lines, keyLine, 0);
  if (!app || !group || !group.name.startsWith("apps-")) {
    throw new Error("Cursor must be inside apps-*.<app> block");
  }

  if (app.name === newKey) {
    throw new Error("New key is the same as current key");
  }

  lines[app.line] = replaceKeyName(lines[app.line], newKey);

  let renamedInReleases = 0;
  const releases = findGlobalReleasesBlock(lines);
  if (releases) {
    for (let i = releases.start; i < releases.end; i += 1) {
      const parsed = parseKey(lines[i]);
      if (parsed && parsed.indent === 6 && parsed.name === app.name) {
        lines[i] = replaceKeyName(lines[i], newKey);
        renamedInReleases += 1;
      }
    }
  }

  return {
    updatedText: lines.join("\n"),
    details: `renamed ${group.name}.${app.name} -> ${newKey}; updated global.releases: ${renamedInReleases}`,
  };
}

function upsertAppInclude(lines: string[], appLine: number, includeName: string): string[] {
  const appEnd = findBlockEnd(lines, appLine + 1, 2);

  for (let i = appLine + 1; i < appEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === 4 && key.name === "_include") {
      const includeEnd = findBlockEnd(lines, i + 1, 4);
      for (let j = i + 1; j < includeEnd; j += 1) {
        const item = lines[j].match(/^\s*-\s+(.+)\s*$/);
        if (item && item[1].trim() === includeName) {
          return lines;
        }
      }
      lines.splice(includeEnd, 0, `      - ${includeName}`);
      return lines;
    }
  }

  lines.splice(appLine + 1, 0, "    _include:", `      - ${includeName}`);
  return lines;
}

function upsertGlobalIncludeProfile(
  lines: string[],
  includeName: string,
  extractedBlock: string[],
  sourceIndent: number,
): string[] {
  const block = extractedBlock.map((line) => {
    if (line.startsWith(" ".repeat(sourceIndent))) {
      return `      ${line.slice(sourceIndent)}`;
    }
    return `      ${line.trimStart()}`;
  });

  const globalLine = findKeyLineByIndent(lines, "global", 0);
  if (globalLine < 0) {
    return [
      "global:",
      "  _includes:",
      `    ${includeName}:`,
      ...block,
      "",
      ...lines,
    ];
  }

  const globalEnd = findBlockEnd(lines, globalLine + 1, 0);
  let includesLine = -1;
  for (let i = globalLine + 1; i < globalEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === 2 && key.name === "_includes") {
      includesLine = i;
      break;
    }
  }

  if (includesLine < 0) {
    lines.splice(globalLine + 1, 0, "  _includes:", `    ${includeName}:`, ...block);
    return lines;
  }

  const includesEnd = findBlockEnd(lines, includesLine + 1, 2);
  for (let i = includesLine + 1; i < includesEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === 4 && key.name === includeName) {
      return lines;
    }
  }

  lines.splice(includesEnd, 0, `    ${includeName}:`, ...block);
  return lines;
}

function findGlobalReleasesBlock(lines: string[]): { start: number; end: number } | null {
  const globalLine = findKeyLineByIndent(lines, "global", 0);
  if (globalLine < 0) {
    return null;
  }
  const globalEnd = findBlockEnd(lines, globalLine + 1, 0);
  for (let i = globalLine + 1; i < globalEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === 2 && key.name === "releases") {
      return { start: i + 1, end: findBlockEnd(lines, i + 1, 2) };
    }
  }
  return null;
}

function findNearestKeyLine(lines: string[], from: number): number {
  const upper = Math.min(Math.max(from, 0), lines.length - 1);
  for (let i = upper; i >= 0; i -= 1) {
    if (parseKey(lines[i])) {
      return i;
    }
  }
  return -1;
}

function findAncestorWithIndent(lines: string[], fromLine: number, targetIndent: number): { line: number; name: string } | null {
  for (let i = fromLine; i >= 0; i -= 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === targetIndent) {
      return { line: i, name: key.name };
    }
  }
  return null;
}

function findKeyLineByIndent(lines: string[], name: string, indent: number): number {
  for (let i = 0; i < lines.length; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === indent && key.name === name) {
      return i;
    }
  }
  return -1;
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

function parseKey(line: string): { indent: number; name: string; tail: string } | null {
  const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  if (!m) {
    return null;
  }
  return { indent: m[1].length, name: m[2], tail: m[3] ?? "" };
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}

function replaceKeyName(line: string, newKey: string): string {
  const parsed = parseKey(line);
  if (!parsed) {
    return line;
  }
  const indentPrefix = " ".repeat(parsed.indent);
  const suffix = parsed.tail.length > 0 ? ` ${parsed.tail}` : "";
  return `${indentPrefix}${newKey}:${suffix}`;
}
