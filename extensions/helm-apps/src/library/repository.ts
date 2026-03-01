export function resolveHelmRepositoryURL(input: string): string {
  const trimmed = input.trim();
  if (trimmed.length === 0) {
    throw new Error("library repository URL is empty");
  }
  const ghMatch = trimmed.match(/github\.com[/:]([^/]+)\/([^/.]+)(?:\.git)?\/?$/i);
  if (ghMatch) {
    return `https://${ghMatch[1]}.github.io/${ghMatch[2]}`;
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return trimmed.replace(/\/+$/g, "");
  }
  throw new Error(`unsupported repository URL: ${trimmed}`);
}

export function compareSemver(a: string, b: string): number {
  const pa = normalizeSemverParts(a);
  const pb = normalizeSemverParts(b);
  for (let i = 0; i < 3; i += 1) {
    if (pa[i] > pb[i]) {
      return 1;
    }
    if (pa[i] < pb[i]) {
      return -1;
    }
  }
  return 0;
}

export function normalizeSemverParts(input: string): [number, number, number] {
  const cleaned = input.replace(/^v/, "").split("-")[0];
  const parts = cleaned.split(".");
  const major = Number.parseInt(parts[0] ?? "0", 10) || 0;
  const minor = Number.parseInt(parts[1] ?? "0", 10) || 0;
  const patch = Number.parseInt(parts[2] ?? "0", 10) || 0;
  return [major, minor, patch];
}
