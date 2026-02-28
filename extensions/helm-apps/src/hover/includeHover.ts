export function extractLocalIncludeBlock(text: string, includeName: string): string | null {
  const lines = text.split(/\r?\n/);
  let inGlobal = false;
  let inIncludes = false;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }

    const indent = m[1].length;
    const key = m[2];

    if (indent === 0) {
      inGlobal = key === "global";
      inIncludes = false;
      continue;
    }

    if (inGlobal && indent === 2) {
      inIncludes = key === "_includes";
      continue;
    }

    if (inGlobal && inIncludes && indent === 4 && key === includeName) {
      const end = findBlockEnd(lines, i + 1, 4);
      const block = lines.slice(i, end).join("\n");
      return stripIndent(block, 4);
    }

    if (inGlobal && indent <= 2 && key !== "_includes") {
      inIncludes = false;
    }
  }

  return null;
}

export function trimPreview(text: string, maxLines = 80, maxChars = 8000): string {
  let out = text;
  const lines = out.split(/\r?\n/);
  if (lines.length > maxLines) {
    out = `${lines.slice(0, maxLines).join("\n")}\n# ...truncated (${lines.length - maxLines} lines omitted)`;
  }
  if (out.length > maxChars) {
    out = `${out.slice(0, maxChars)}\n# ...truncated`;
  }
  return out;
}

function findBlockEnd(lines: string[], start: number, ownerIndent: number): number {
  for (let i = start; i < lines.length; i += 1) {
    const t = lines[i].trim();
    if (t.length === 0 || t.startsWith("#")) {
      continue;
    }
    const ind = countIndent(lines[i]);
    if (ind <= ownerIndent) {
      return i;
    }
  }
  return lines.length;
}

function stripIndent(block: string, indent: number): string {
  const prefix = " ".repeat(indent);
  return block
    .split(/\r?\n/)
    .map((l) => (l.startsWith(prefix) ? l.slice(indent) : l))
    .join("\n");
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}
