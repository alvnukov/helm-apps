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

  const app = findAncestorWithIndent(lines, keyLine, 2);
  const group = findAncestorWithIndent(lines, keyLine, 0);
  if (!app || !group || !group.name.startsWith("apps-")) {
    throw new Error("Key must be inside apps-*.<app> scope");
  }
  const appKey = parseKey(lines[app.line]);
  if (!appKey) {
    throw new Error("Key must be inside apps-*.<app> scope");
  }

  const chain = findKeyChainWithinApp(lines, keyLine, app.line, appKey.indent);
  if (!chain || chain.length === 0) {
    throw new Error("Place cursor on app child key to extract");
  }
  const key = chain[chain.length - 1];
  const owner = chain.length >= 2 ? chain[chain.length - 2] : { line: app.line, indent: appKey.indent, name: app.name };
  if (key.name === "_include") {
    throw new Error("Cannot extract _include key");
  }

  const blockEnd = findBlockEnd(lines, key.line + 1, key.indent);
  const extracted = lines.slice(key.line, blockEnd);

  const withoutBlock = [...lines.slice(0, key.line), ...lines.slice(blockEnd)];
  const withInclude = upsertIncludeAtOwner(withoutBlock, owner.line, owner.indent, includeName);
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

function upsertIncludeAtOwner(lines: string[], ownerLine: number, ownerIndent: number, includeName: string): string[] {
  const ownerEnd = findBlockEnd(lines, ownerLine + 1, ownerIndent);
  const childIndent = ownerIndent + 2;
  const includeIndent = childIndent + 2;

  for (let i = ownerLine + 1; i < ownerEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (key && key.indent === childIndent && key.name === "_include") {
      const includeEnd = findBlockEnd(lines, i + 1, childIndent);
      const existing = normalizeIncludeTail(key.tail);
      for (let j = i + 1; j < includeEnd; j += 1) {
        const item = lines[j].match(/^\s*-\s+(.+)\s*$/);
        if (item) {
          const candidate = unquote(item[1].trim());
          if (isIncludeToken(candidate)) {
            existing.push(candidate);
          }
        }
      }
      if (!existing.includes(includeName)) {
        existing.push(includeName);
      }
      const deduped = [...new Set(existing)];
      const replacement = [
        `${" ".repeat(childIndent)}_include:`,
        ...deduped.map((name) => `${" ".repeat(includeIndent)}- ${name}`),
      ];
      lines.splice(i, includeEnd - i, ...replacement);
      return lines;
    }
  }

  lines.splice(
    ownerLine + 1,
    0,
    `${" ".repeat(childIndent)}_include:`,
    `${" ".repeat(includeIndent)}- ${includeName}`,
  );
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
      return line.slice(sourceIndent);
    }
    return line.trimStart();
  }).map((line) => `      ${line}`);

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
      return mergeIntoIncludeProfile(lines, i, includeName, block);
    }
  }

  lines.splice(includesEnd, 0, `    ${includeName}:`, ...block);
  return lines;
}

function mergeIntoIncludeProfile(
  lines: string[],
  includeLine: number,
  includeName: string,
  block: string[],
): string[] {
  const includeEnd = findBlockEnd(lines, includeLine + 1, 4);
  const incomingRootKey = findFirstKeyAtIndent(block, 6);
  if (!incomingRootKey) {
    lines.splice(includeEnd, 0, ...block);
    return lines;
  }

  for (let i = includeLine + 1; i < includeEnd; i += 1) {
    const key = parseKey(lines[i]);
    if (!key || key.indent !== 6) {
      continue;
    }
    if (key.name === incomingRootKey) {
      throw new Error(`Include profile '${includeName}' already contains key '${incomingRootKey}'`);
    }
  }

  lines.splice(includeEnd, 0, ...block);
  return lines;
}

function findFirstKeyAtIndent(lines: string[], indent: number): string | null {
  for (const line of lines) {
    const key = parseKey(line);
    if (key && key.indent === indent) {
      return key.name;
    }
  }
  return null;
}

function normalizeIncludeTail(tail: string): string[] {
  const trimmed = tail.trim();
  if (trimmed.length === 0) {
    return [];
  }
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    const inside = trimmed.slice(1, -1);
    return inside
      .split(",")
      .map((part) => unquote(part.trim()))
      .filter((name) => isIncludeToken(name));
  }
  const one = unquote(trimmed);
  return isIncludeToken(one) ? [one] : [];
}

function isIncludeToken(value: string): boolean {
  return /^[A-Za-z0-9_.-]+$/.test(value);
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length > 1 && ((v.startsWith("\"") && v.endsWith("\"")) || (v.startsWith("'") && v.endsWith("'")))) {
    return v.slice(1, -1);
  }
  return v;
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
    if (isLineInsideBlockScalar(lines, i)) {
      continue;
    }
    if (parseKey(lines[i])) {
      return i;
    }
  }
  return -1;
}

function findDirectAppChildLine(lines: string[], startLine: number, appLine: number, appIndent: number): number {
  const childIndent = appIndent + 2;
  for (let i = startLine; i >= appLine + 1; i -= 1) {
    const key = parseKey(lines[i]);
    if (!key) {
      continue;
    }
    if (key.indent === childIndent) {
      return i;
    }
    if (key.indent <= appIndent) {
      break;
    }
  }
  return -1;
}

function findKeyChainWithinApp(
  lines: string[],
  keyLine: number,
  appLine: number,
  appIndent: number,
): Array<{ line: number; indent: number; name: string }> | null {
  const key = parseKey(lines[keyLine]);
  if (!key || key.indent <= appIndent) {
    return null;
  }
  const chain: Array<{ line: number; indent: number; name: string }> = [{ line: keyLine, indent: key.indent, name: key.name }];
  let currentIndent = key.indent;
  let cursor = keyLine - 1;

  while (cursor > appLine) {
    const parent = findParentKeyAbove(lines, cursor, currentIndent, appLine, appIndent);
    if (!parent) {
      break;
    }
    chain.push(parent);
    currentIndent = parent.indent;
    cursor = parent.line - 1;
    if (currentIndent === appIndent + 2) {
      break;
    }
  }

  if (chain[chain.length - 1].indent !== appIndent + 2) {
    return null;
  }
  return chain.reverse();
}

function findParentKeyAbove(
  lines: string[],
  startLine: number,
  childIndent: number,
  appLine: number,
  appIndent: number,
): { line: number; indent: number; name: string } | null {
  for (let i = startLine; i > appLine; i -= 1) {
    if (isLineInsideBlockScalar(lines, i)) {
      continue;
    }
    const key = parseKey(lines[i]);
    if (!key) {
      continue;
    }
    if (key.indent < childIndent && key.indent > appIndent) {
      return { line: i, indent: key.indent, name: key.name };
    }
    if (key.indent <= appIndent) {
      return null;
    }
  }
  return null;
}

function findAncestorWithIndent(lines: string[], fromLine: number, targetIndent: number): { line: number; name: string } | null {
  for (let i = fromLine; i >= 0; i -= 1) {
    if (isLineInsideBlockScalar(lines, i)) {
      continue;
    }
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

function isLineInsideBlockScalar(lines: string[], lineIndex: number): boolean {
  if (lineIndex < 0 || lineIndex >= lines.length) {
    return false;
  }
  const lineIndent = countIndent(lines[lineIndex]);
  let scalarOwnerIndent = -1;

  for (let i = 0; i <= lineIndex; i += 1) {
    const key = parseKey(lines[i]);
    if (!key) {
      continue;
    }

    if (scalarOwnerIndent >= 0 && key.indent <= scalarOwnerIndent) {
      scalarOwnerIndent = -1;
    }

    if (i === lineIndex) {
      break;
    }

    const tail = key.tail.trim();
    if (/^[|>][-+0-9]*$/.test(tail)) {
      scalarOwnerIndent = key.indent;
    }
  }

  return scalarOwnerIndent >= 0 && lineIndent > scalarOwnerIndent;
}
