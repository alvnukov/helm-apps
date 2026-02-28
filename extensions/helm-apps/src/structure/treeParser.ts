export interface ValuesNode {
  label: string;
  line: number;
  children: ValuesNode[];
  path: string;
}

export function parseYamlKeyTree(text: string): ValuesNode[] {
  const lines = text.split(/\r?\n/);
  const roots: ValuesNode[] = [];
  const stack: Array<{ indent: number; node: ValuesNode }> = [];

  let blockScalarIndent: number | null = null;

  for (let i = 0; i < lines.length; i += 1) {
    const raw = lines[i];
    const trimmed = raw.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }

    const indent = countIndent(raw);

    if (blockScalarIndent !== null) {
      if (indent > blockScalarIndent) {
        continue;
      }
      blockScalarIndent = null;
    }

    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }

    const keyMatch = raw.match(/^\s*([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!keyMatch) {
      continue;
    }

    const key = keyMatch[1];
    const tail = keyMatch[2].trim();

    const pathParts = stack.map((entry) => entry.node.label).concat(key);
    const node: ValuesNode = {
      label: key,
      line: i,
      children: [],
      path: pathParts.join("."),
    };

    if (stack.length === 0) {
      roots.push(node);
    } else {
      stack[stack.length - 1].node.children.push(node);
    }

    stack.push({ indent, node });

    if (/^[|>][-+]?\s*$/.test(tail)) {
      blockScalarIndent = indent;
    }
  }

  return roots;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}
