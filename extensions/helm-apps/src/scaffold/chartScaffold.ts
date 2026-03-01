export interface StarterChartConfig {
  chartName: string;
  chartVersion: string;
  libraryVersion: string;
}

export function sanitizeChartName(input: string): string {
  const trimmed = input.trim().toLowerCase();
  const replaced = trimmed.replace(/[^a-z0-9.-]+/g, "-").replace(/-+/g, "-").replace(/^-+|-+$/g, "");
  return replaced.length > 0 ? replaced : "app";
}

export function isValidChartVersion(version: string): boolean {
  return /^\d+\.\d+\.\d+([.-][A-Za-z0-9.-]+)?$/.test(version.trim());
}

export function buildStarterChartFiles(cfg: StarterChartConfig): Record<string, string> {
  const chartName = sanitizeChartName(cfg.chartName);
  const version = cfg.chartVersion.trim();
  const libraryVersion = cfg.libraryVersion.trim();

  const chartYaml = [
    "apiVersion: v2",
    `name: ${chartName}`,
    "description: Starter chart with vendored helm-apps library",
    "type: application",
    `version: ${version}`,
    'appVersion: "1.0.0"',
    "dependencies:",
    "  - name: helm-apps",
    `    version: \"${libraryVersion}\"`,
    "",
  ].join("\n");

  const valuesYaml = [
    "global:",
    "  env: dev",
    "",
    "apps-stateless:",
    "  app-1:",
    "    enabled: true",
    "    containers:",
    "      app-1:",
    "        image:",
    "          name: nginx",
    '          staticTag: "latest"',
    "        ports: |-",
    "          - name: http",
    "            containerPort: 80",
    "    service:",
    "      enabled: true",
    "      ports: |-",
    "        - name: http",
    "          port: 80",
    "",
  ].join("\n");

  const initTpl = '{{- include "apps-utils.init-library" $ }}\n';

  return {
    "Chart.yaml": chartYaml,
    "values.yaml": valuesYaml,
    "templates/init-helm-apps-library.yaml": initTpl,
  };
}
