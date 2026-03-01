import * as path from "node:path";

export interface CommandInvocation {
  cmd: string;
  args: string[];
}

export function buildHelmCommandCandidates(configuredHelmPath: string, helmArgs: string[]): CommandInvocation[] {
  const configured = configuredHelmPath.trim();
  const candidates: CommandInvocation[] = [];
  if (configured.length === 0 || configured === "helm") {
    candidates.push({ cmd: "helm", args: helmArgs });
  } else if (path.basename(configured) === "werf") {
    candidates.push({ cmd: configured, args: ["helm", ...helmArgs] });
  } else {
    candidates.push({ cmd: configured, args: helmArgs });
  }
  candidates.push({ cmd: "werf", args: ["helm", ...helmArgs] });

  const unique: CommandInvocation[] = [];
  const seen = new Set<string>();
  for (const candidate of candidates) {
    const key = `${candidate.cmd}\u0000${candidate.args.join("\u0000")}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    unique.push(candidate);
  }
  return unique;
}
