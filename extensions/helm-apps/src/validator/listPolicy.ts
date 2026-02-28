import type { ValidationIssue, ValidationOptions } from "./types";

const BUILTIN_LIST_FIELDS = new Set([
  "accessModes",
  "args",
  "command",
  "ports",
  "tolerations",
  "imagePullSecrets",
  "hostAliases",
  "topologySpreadConstraints",
  "clusterIPs",
  "externalIPs",
  "ipFamilies",
  "loadBalancerSourceRanges",
  "extraGroups",
  "nodeGroups",
  "sshPublicKeys",
  "volumes",
  "volumeClaimTemplates",
]);

export function validateUnexpectedNativeLists(text: string, options: ValidationOptions = {}): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const stack: Array<{ indent: number; key: string }> = [];

  let blockScalarIndent: number | null = null;

  const lines = text.split(/\r?\n/);
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
    if (keyMatch) {
      const key = keyMatch[1];
      const tail = keyMatch[2].trim();
      stack.push({ indent, key });

      if (/^[|>][-+]?\s*$/.test(tail)) {
        blockScalarIndent = indent;
      }
      continue;
    }

    if (/^\s*-\s+/.test(raw)) {
      const path = toValuesPath(stack.map((s) => s.key));
      const key = stack.length > 0 ? stack[stack.length - 1].key : "";

      if (!isAllowedListPath(path, key, !!options.allowNativeListsInBuiltInListFields)) {
        issues.push({
          code: "E_UNEXPECTED_LIST",
          message: "native YAML list is not allowed here",
          path,
          line: i + 1,
        });
      }
    }
  }

  return issues;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}

function toValuesPath(keys: string[]): string {
  if (keys.length === 0) {
    return "Values";
  }
  return `Values.${keys.join(".")}`;
}

export function isAllowedListPath(path: string, key: string, allowBuiltInLists: boolean): boolean {
  if (key === "_include" || key === "_include_files") {
    return true;
  }

  if (/^Values\.global\._includes\..*/.test(path)) {
    return true;
  }
  if (/^Values\.apps-kafka-strimzi\..*\.kafka\.brokers\.hosts\.[^.]+$/.test(path)) {
    return true;
  }
  if (/^Values\.apps-kafka-strimzi\..*\.kafka\.ui\.dex\.allowedGroups\.[^.]+$/.test(path)) {
    return true;
  }
  if (/^Values\..*\.configFilesYAML\..*\.content\..*/.test(path)) {
    return true;
  }
  if (/^Values\..*\.envYAML\..*/.test(path)) {
    return true;
  }
  if (/^Values\..*\.extraFields(\..*)?$/.test(path)) {
    return true;
  }
  if (/^Values\.apps-service-accounts\.[^.]+\.(roles|clusterRoles)\.[^.]+\.rules\.[^.]+\.(apiGroups|resources|verbs|resourceNames|nonResourceURLs)$/.test(path)) {
    return true;
  }
  if (/^Values\.apps-service-accounts\.[^.]+\.(roles|clusterRoles)\.[^.]+\.binding\.subjects$/.test(path)) {
    return true;
  }
  if (/^Values\..*\.containers\.[^.]+\.sharedEnvConfigMaps$/.test(path)) {
    return true;
  }
  if (/^Values\..*\.initContainers\.[^.]+\.sharedEnvConfigMaps$/.test(path)) {
    return true;
  }
  if (/^Values\..*\.containers\.[^.]+\.sharedEnvSecrets$/.test(path)) {
    return true;
  }
  if (/^Values\..*\.initContainers\.[^.]+\.sharedEnvSecrets$/.test(path)) {
    return true;
  }

  if (allowBuiltInLists && BUILTIN_LIST_FIELDS.has(key)) {
    return true;
  }

  return false;
}
