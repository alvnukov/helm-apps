import { readFileSync } from "node:fs";
import path from "node:path";

export interface FieldDoc {
  title: string;
  summary: string;
  type: string;
  notes?: string[];
  example?: string;
  docsLink?: string;
  docsLinkEn?: string;
  docsLinkRu?: string;
  k8sDocsLink?: string;
  titleRu?: string;
  summaryRu?: string;
  typeRu?: string;
  notesRu?: string[];
}

interface DocRule {
  pattern: string[];
  doc: FieldDoc;
}

type JsonSchema = {
  $ref?: string;
  description?: string;
  type?: string | string[];
  enum?: unknown[];
  default?: unknown;
  examples?: unknown[];
  properties?: Record<string, JsonSchema>;
  patternProperties?: Record<string, JsonSchema>;
  additionalProperties?: boolean | JsonSchema;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  allOf?: JsonSchema[];
  $defs?: Record<string, JsonSchema>;
};

let schemaRootCache: JsonSchema | null = null;
const DOCS_ONLINE_BASE = "https://github.com/alvnukov/helm-apps/blob/main/";

const BUILTIN_GROUPS = new Set([
  "apps-stateless",
  "apps-stateful",
  "apps-jobs",
  "apps-cronjobs",
  "apps-services",
  "apps-ingresses",
  "apps-network-policies",
  "apps-configmaps",
  "apps-secrets",
  "apps-pvcs",
  "apps-limit-range",
  "apps-certificates",
  "apps-dex-clients",
  "apps-dex-authenticators",
  "apps-custom-prometheus-rules",
  "apps-grafana-dashboards",
  "apps-kafka-strimzi",
  "apps-infra",
  "apps-service-accounts",
  "apps-k8s-manifests",
]);

const RULES: DocRule[] = [
  {
    pattern: ["global", "env"],
    doc: {
      title: "Environment Selector",
      titleRu: "Выбор окружения",
      summary: "Selects active environment for env-maps (`_default`, `prod`, regex keys).",
      summaryRu: "Выбирает активное окружение для env-map (`_default`, `prod`, regex-ключи).",
      type: "string",
      notes: [
        "Used across all app groups for env-specific values.",
        "Can be any string, not limited to discovered envs.",
      ],
      notesRu: [
        "Используется во всех app-группах для env-специфичных значений.",
        "Может быть любой строкой, не ограничивается обнаруженными env.",
      ],
      example: "global:\n  env: prod\n",
    },
  },
  {
    pattern: ["global", "_includes"],
    doc: {
      title: "Include Profiles Registry",
      titleRu: "Реестр include-профилей",
      summary: "Reusable profiles merged by `_include` in groups/apps.",
      summaryRu: "Переиспользуемые профили, которые мержатся через `_include` в группах/приложениях.",
      type: "map",
      docsLink: "docs/reference-values.md#param-global-includes",
      notes: [
        "Profiles merge recursively.",
        "App-local values override included profile values.",
      ],
      notesRu: [
        "Профили мержатся рекурсивно.",
        "Локальные значения приложения переопределяют include-профили.",
      ],
      example: "global:\n  _includes:\n    apps-default:\n      enabled: true\n",
    },
  },
  {
    pattern: ["*", "__GroupVars__", "type"],
    doc: {
      title: "Group Renderer Type",
      titleRu: "Тип рендерера группы",
      summary: "Defines renderer for a custom group (built-in `apps-*` or custom renderer type).",
      summaryRu: "Определяет рендерер для кастомной группы (встроенный `apps-*` или пользовательский тип).",
      type: "string | env-map",
      notes: [
        "For custom renderer, define template `<type>.render` in chart templates.",
      ],
      notesRu: [
        "Для кастомного рендерера определите шаблон `<type>.render` в templates чарта.",
      ],
      example: "custom-group:\n  __GroupVars__:\n    type: apps-stateless\n",
    },
  },
  {
    pattern: ["*", "*", "enabled"],
    doc: {
      title: "Resource Toggle",
      summary: "Enables/disables app rendering.",
      type: "bool | env-map",
      notes: [
        "In release-matrix mode may be auto-enabled by `global.deploy.enabled`.",
      ],
      example: "apps-stateless:\n  api:\n    enabled: true\n",
    },
  },
  {
    pattern: ["*", "*", "_include"],
    doc: {
      title: "Include Profiles",
      titleRu: "Подключение include-профилей",
      summary: "Applies one or more profiles from `global._includes` to current app/group.",
      summaryRu: "Применяет один или несколько профилей из `global._includes` к текущему app/group.",
      type: "string | string[]",
      docsLink: "docs/reference-values.md#param-include",
      notes: [
        "Chains are concatenated in declared order.",
        "Local fields override included values.",
      ],
      notesRu: [
        "Цепочки конкатенируются в указанном порядке.",
        "Локальные поля переопределяют значения из include.",
      ],
      example: "apps-stateless:\n  api:\n    _include: [apps-default]\n",
    },
  },
  {
    pattern: ["*", "*", "_include_from_file"],
    doc: {
      title: "Inline Include From File",
      titleRu: "Inline include из файла",
      summary: "Loads YAML map from file and merges it into current object.",
      summaryRu: "Загружает YAML map из файла и мержит его в текущий объект.",
      type: "string",
      docsLink: "docs/reference-values.md#param-include-from-file",
      notes: [
        "Path is relative to current values file.",
        "Missing file is skipped (warning only).",
      ],
      notesRu: [
        "Путь относительный к текущему values-файлу.",
        "Отсутствующий файл пропускается (только предупреждение).",
      ],
      example: "_include_from_file: helm-apps-defaults.yaml\n",
    },
  },
  {
    pattern: ["*", "*", "_include_files"],
    doc: {
      title: "Import Include Profiles From Files",
      titleRu: "Импорт include-профилей из файлов",
      summary: "Loads file maps as named include profiles and prepends them into `_include`.",
      summaryRu: "Загружает файлы как именованные include-профили и добавляет их в начало `_include`.",
      type: "string[]",
      docsLink: "docs/reference-values.md#param-include-files",
      notes: [
        "Each file becomes profile named by filename.",
        "Supports navigation/hover to include definition.",
      ],
      notesRu: [
        "Каждый файл становится профилем с именем по файлу.",
        "Поддерживается переход/hover к определению include.",
      ],
      example: "_include_files:\n  - defaults.yaml\n  - profile-prod.yaml\n",
    },
  },
  {
    pattern: ["*", "*", "labels"],
    doc: {
      title: "Labels Override",
      summary: "Kubernetes labels fragment for rendered resource.",
      type: "YAML block string | env-map",
      notes: [
        "Prefer YAML block (`|-`) for map content to follow library conventions.",
      ],
      example: "labels: |-\n  app.kubernetes.io/component: api\n",
    },
  },
  {
    pattern: ["*", "*", "annotations"],
    doc: {
      title: "Annotations Override",
      summary: "Kubernetes annotations fragment for rendered resource.",
      type: "YAML block string | env-map",
      example: "annotations: |-\n  nginx.ingress.kubernetes.io/proxy-body-size: 64m\n",
    },
  },
  {
    pattern: ["*", "*", "containers"],
    doc: {
      title: "Containers Spec",
      summary: "Container definitions for workload groups.",
      type: "map | YAML block string",
      notes: [
        "For complex k8s snippets prefer block-string fields where group contract expects it.",
      ],
      example: "containers:\n  app:\n    image:\n      name: nginx\n",
    },
  },
  {
    pattern: ["*", "*", "service"],
    doc: {
      title: "Service Settings",
      summary: "Service exposure settings bound to app workload.",
      type: "map",
      notes: [
        "Commonly used in `apps-stateless`/`apps-stateful`.",
      ],
      example: "service:\n  enabled: true\n  ports: |-\n    - name: http\n      port: 80\n",
    },
  },
  {
    pattern: ["*", "*", "ports"],
    doc: {
      title: "Ports Configuration",
      summary: "Port list for service/container depending on field context.",
      type: "YAML block string",
      notes: [
        "Native YAML list may be restricted by list-policy.",
      ],
      example: "ports: |-\n  - name: http\n    port: 80\n",
    },
  },
  {
    pattern: ["*", "*", "envYAML"],
    doc: {
      title: "Container Env List",
      summary: "Environment variables in Kubernetes `env` list format.",
      type: "list",
      docsLink: "docs/reference-values.md#param-envyaml",
      notes: [
        "One of allowed native-list paths in list-policy validator.",
      ],
      example: "envYAML:\n  - name: LOG_LEVEL\n    value: info\n",
    },
  },
  {
    pattern: ["*", "*", "fieldsYAML"],
    doc: {
      title: "Raw Fields YAML",
      summary: "Additional raw fields merged into rendered resource body.",
      type: "map of YAML block strings",
      notes: [
        "Used heavily by fallback and k8s-manifest style resources.",
      ],
      example: "fieldsYAML:\n  spec: |-\n    template:\n      spec:\n        hostNetwork: true\n",
    },
  },
  {
    pattern: ["apps-k8s-manifests", "*", "extraFields"],
    doc: {
      title: "Residual Top-level Fields",
      summary: "Unknown/unmapped top-level fields preserved for generic manifest rendering.",
      type: "YAML block string",
      example: "extraFields: |-\n  roleRef:\n    kind: Role\n",
    },
  },
  {
    pattern: ["*", "*", "resources"],
    doc: {
      title: "Resources",
      titleRu: "Ресурсы",
      summary: "CPU/Memory resource profile for the component.",
      summaryRu: "Профиль CPU/Memory ресурсов для компонента.",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-resources",
      k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
      notes: [
        "Commonly contains `requests` and `limits`.",
        "Can be inherited via `_include` profiles.",
      ],
      notesRu: [
        "Обычно содержит `requests` и `limits`.",
        "Может наследоваться через `_include` профили.",
      ],
      example: "resources:\n  requests:\n    mcpu: 100\n    memoryMb: 256\n  limits:\n    mcpu: 500\n    memoryMb: 512\n",
    },
  },
  {
    pattern: ["*", "*", "*", "resources"],
    doc: {
      title: "Resources",
      titleRu: "Ресурсы",
      summary: "CPU/Memory resource profile for nested component (for example in kafka/strimzi blocks).",
      summaryRu: "Профиль CPU/Memory ресурсов для вложенного компонента (например в блоках kafka/strimzi).",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-resources",
      k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
      notes: [
        "Typical nested keys: `requests`, `limits`.",
      ],
      notesRu: [
        "Типичные вложенные ключи: `requests`, `limits`.",
      ],
      example: "kafka:\n  resources:\n    requests:\n      mcpu: 100\n      memoryMb: 768\n    limits:\n      mcpu: 1000\n      memoryMb: 4096\n",
    },
  },
  {
    pattern: ["*", "*", "*", "resources", "requests"],
    doc: {
      title: "Resource Requests",
      summary: "Guaranteed resources requested by scheduler.",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-resources",
      k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
      example: "requests:\n  mcpu: 100\n  memoryMb: 768\n",
    },
  },
  {
    pattern: ["*", "*", "*", "resources", "limits"],
    doc: {
      title: "Resource Limits",
      summary: "Maximum resources allowed for the component.",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-resources",
      k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
      example: "limits:\n  mcpu: 1000\n  memoryMb: 4096\n",
    },
  },
  {
    pattern: ["*", "*", "*", "envVars"],
    doc: {
      title: "envVars Helper",
      titleRu: "Хелпер envVars",
      summary: "Declares container env variables as key/value map with env-map support.",
      summaryRu: "Описывает env-переменные контейнера как map key/value с поддержкой env-map.",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-envvars-usage",
      example: "envVars:\n  LOG_LEVEL: info\n  APP_MODE:\n    _default: safe\n    production: fast\n",
    },
  },
  {
    pattern: ["*", "*", "*", "secretEnvVars"],
    doc: {
      title: "secretEnvVars Helper",
      titleRu: "Хелпер secretEnvVars",
      summary: "Generates Secret-backed env vars and wires them into container env chain.",
      summaryRu: "Генерирует env-переменные через Secret и подключает их в env-цепочку контейнера.",
      type: "map | env-map",
      docsLink: "docs/reference-values.md#param-secretenvvars",
      example: "secretEnvVars:\n  DB_PASSWORD:\n    _default: dev-password\n    production: prod-password\n",
    },
  },
  {
    pattern: ["*", "*", "*", "sharedEnvSecrets"],
    doc: {
      title: "sharedEnvSecrets Helper",
      titleRu: "Хелпер sharedEnvSecrets",
      summary: "Adds shared Secret references into container `envFrom`.",
      summaryRu: "Добавляет ссылки на общие Secret в `envFrom` контейнера.",
      type: "list",
      docsLink: "docs/reference-values.md#param-sharedenvsecrets",
      example: "sharedEnvSecrets:\n  - common-runtime\n  - platform-observability\n",
    },
  },
  {
    pattern: ["*", "*", "*", "sharedEnvConfigMaps"],
    doc: {
      title: "sharedEnvConfigMaps Helper",
      titleRu: "Хелпер sharedEnvConfigMaps",
      summary: "Adds shared ConfigMap references into container `envFrom`.",
      summaryRu: "Добавляет ссылки на общие ConfigMap в `envFrom` контейнера.",
      type: "list",
      docsLink: "docs/reference-values.md#param-sharedenvconfigmaps",
      example: "sharedEnvConfigMaps:\n  - common-runtime-cm\n  - platform-observability-cm\n",
    },
  },
  {
    pattern: ["*", "*", "*", "configFiles"],
    doc: {
      title: "configFiles Helper",
      titleRu: "Хелпер configFiles",
      summary: "Creates/mounts text config files for a container.",
      summaryRu: "Создает/монтирует текстовые конфигурационные файлы для контейнера.",
      type: "map",
      docsLink: "docs/reference-values.md#param-configfiles",
      example: "configFiles:\n  app.yaml:\n    mountPath: /etc/app/app.yaml\n    content: |\n      key: value\n",
    },
  },
  {
    pattern: ["*", "*", "*", "configFilesYAML"],
    doc: {
      title: "configFilesYAML Helper",
      titleRu: "Хелпер configFilesYAML",
      summary: "YAML-aware config files with env-map overrides in content tree.",
      summaryRu: "YAML-конфиги с env-map переопределениями внутри дерева content.",
      type: "map",
      docsLink: "docs/reference-values.md#param-configfilesyaml",
      example: "configFilesYAML:\n  app.yaml:\n    mountPath: /etc/app/app.yaml\n    content:\n      key:\n        _default: value\n        production: prod-value\n",
    },
  },
  {
    pattern: ["*", "*", "*", "fromSecretsEnvVars"],
    doc: {
      title: "fromSecretsEnvVars Helper",
      titleRu: "Хелпер fromSecretsEnvVars",
      summary: "Maps keys from external Secret into explicit env vars.",
      summaryRu: "Маппит ключи из внешнего Secret в явные env-переменные.",
      type: "map | list",
      docsLink: "docs/reference-values.md#param-fromsecretsenvvars",
      example: "fromSecretsEnvVars:\n  external-db:\n    DB_PASSWORD: password\n",
    },
  },
];

const LAST_KEY_RULES: Record<string, FieldDoc> = {
  resources: {
    title: "Resources",
    summary: "Resource configuration block (requests/limits).",
    type: "map | env-map",
    docsLink: "docs/reference-values.md#param-resources",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
    notes: [
      "Supported by many library entities and nested component blocks.",
    ],
    example: "resources:\n  requests:\n    mcpu: 100\n    memoryMb: 256\n  limits:\n    mcpu: 500\n    memoryMb: 512\n",
  },
  requests: {
    title: "Resource Requests",
    summary: "Requested CPU/Memory values for scheduling.",
    type: "map | env-map",
    docsLink: "docs/reference-values.md#param-resources",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
    example: "requests:\n  mcpu: 100\n  memoryMb: 256\n",
  },
  limits: {
    title: "Resource Limits",
    summary: "Upper bound CPU/Memory values for runtime.",
    type: "map | env-map",
    docsLink: "docs/reference-values.md#param-resources",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
    example: "limits:\n  mcpu: 500\n  memoryMb: 512\n",
  },
  mcpu: {
    title: "CPU (mCPU)",
    summary: "CPU value in millicores.",
    type: "number | env-map",
    docsLink: "docs/reference-values.md#param-resources",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
    notes: ["Example: `100` means 0.1 CPU core."],
    example: "mcpu:\n  _default: 100\n  production: 500\n",
  },
  memoryMb: {
    title: "Memory (MiB)",
    summary: "Memory value in MiB.",
    type: "number | env-map",
    docsLink: "docs/reference-values.md#param-resources",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/",
    example: "memoryMb:\n  _default: 256\n  production: 1024\n",
  },
  envVars: {
    title: "envVars Helper",
    summary: "Declares container env variables as key/value map with env-map support.",
    type: "map | env-map",
    docsLink: "docs/reference-values.md#param-envvars-usage",
    example: "envVars:\n  LOG_LEVEL: info\n  APP_MODE:\n    _default: safe\n    production: fast\n",
  },
  secretEnvVars: {
    title: "secretEnvVars Helper",
    summary: "Generates Secret-backed env vars and wires them into container env chain.",
    type: "map | env-map",
    docsLink: "docs/reference-values.md#param-secretenvvars",
    example: "secretEnvVars:\n  DB_PASSWORD:\n    _default: dev-password\n    production: prod-password\n",
  },
  sharedEnvSecrets: {
    title: "sharedEnvSecrets Helper",
    summary: "Adds shared Secret references into container `envFrom`.",
    type: "list",
    docsLink: "docs/reference-values.md#param-sharedenvsecrets",
    example: "sharedEnvSecrets:\n  - common-runtime\n  - platform-observability\n",
  },
  sharedEnvConfigMaps: {
    title: "sharedEnvConfigMaps Helper",
    summary: "Adds shared ConfigMap references into container `envFrom`.",
    type: "list",
    docsLink: "docs/reference-values.md#param-sharedenvconfigmaps",
    example: "sharedEnvConfigMaps:\n  - common-runtime-cm\n  - platform-observability-cm\n",
  },
  configFiles: {
    title: "configFiles Helper",
    summary: "Creates/mounts text config files for a container.",
    type: "map",
    docsLink: "docs/reference-values.md#param-configfiles",
    example: "configFiles:\n  app.yaml:\n    mountPath: /etc/app/app.yaml\n    content: |\n      key: value\n",
  },
  configFilesYAML: {
    title: "configFilesYAML Helper",
    summary: "YAML-aware config files with env-map overrides in content tree.",
    type: "map",
    docsLink: "docs/reference-values.md#param-configfilesyaml",
    example: "configFilesYAML:\n  app.yaml:\n    mountPath: /etc/app/app.yaml\n    content:\n      key:\n        _default: value\n        production: prod-value\n",
  },
  fromSecretsEnvVars: {
    title: "fromSecretsEnvVars Helper",
    summary: "Maps keys from external Secret into explicit env vars.",
    type: "map | list",
    docsLink: "docs/reference-values.md#param-fromsecretsenvvars",
    example: "fromSecretsEnvVars:\n  external-db:\n    DB_PASSWORD: password\n",
  },
  image: {
    title: "Container Image",
    titleRu: "Образ контейнера",
    summary: "Container image settings (`name`, `staticTag`, optional generated tag behavior).",
    summaryRu: "Настройки образа контейнера (`name`, `staticTag`, опционально генерация тега).",
    type: "map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#image",
    docsLinkRu: "docs/k8s-fields-guide.md#image",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/containers/images/",
    example: "image:\n  name: ghcr.io/example/api\n  staticTag: \"1.2.3\"\n",
  },
  command: {
    title: "Container Command",
    titleRu: "Команда контейнера",
    summary: "Overrides container entrypoint command.",
    summaryRu: "Переопределяет команду запуска контейнера (entrypoint).",
    type: "YAML block string | env-map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#command-and-args",
    docsLinkRu: "docs/k8s-fields-guide.md#command-and-args",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container/",
    example: "command: |-\n  - /app/server\n",
  },
  args: {
    title: "Container Args",
    titleRu: "Аргументы контейнера",
    summary: "Sets arguments passed to container command.",
    summaryRu: "Задает аргументы, передаваемые в команду контейнера.",
    type: "YAML block string | env-map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#command-and-args",
    docsLinkRu: "docs/k8s-fields-guide.md#command-and-args",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/inject-data-application/define-command-argument-container/",
    example: "args: |-\n  - --port=8080\n  - --log-level=info\n",
  },
  livenessProbe: {
    title: "Liveness Probe",
    titleRu: "Liveness Probe",
    summary: "Restart check: if probe fails repeatedly, Kubernetes restarts the container.",
    summaryRu: "Проверка живости: при регулярном провале Kubernetes перезапускает контейнер.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#probes-liveness-readiness-startup",
    docsLinkRu: "docs/k8s-fields-guide.md#probes-liveness-readiness-startup",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/",
    example: "livenessProbe: |-\n  httpGet:\n    path: /healthz\n    port: 8080\n  initialDelaySeconds: 15\n",
  },
  readinessProbe: {
    title: "Readiness Probe",
    titleRu: "Readiness Probe",
    summary: "Traffic readiness check: failing pod is removed from Service endpoints.",
    summaryRu: "Проверка готовности к трафику: при провале pod убирается из endpoint'ов Service.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#probes-liveness-readiness-startup",
    docsLinkRu: "docs/k8s-fields-guide.md#probes-liveness-readiness-startup",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/",
    example: "readinessProbe: |-\n  httpGet:\n    path: /ready\n    port: 8080\n  periodSeconds: 5\n",
  },
  startupProbe: {
    title: "Startup Probe",
    titleRu: "Startup Probe",
    summary: "Slow-start guard: delays liveness/readiness failures until app starts.",
    summaryRu: "Защита медленного старта: откладывает liveness/readiness до запуска приложения.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#probes-liveness-readiness-startup",
    docsLinkRu: "docs/k8s-fields-guide.md#probes-liveness-readiness-startup",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/configure-pod-container/configure-liveness-readiness-startup-probes/",
    example: "startupProbe: |-\n  httpGet:\n    path: /startup\n    port: 8080\n  failureThreshold: 30\n  periodSeconds: 10\n",
  },
  securityContext: {
    title: "Security Context",
    titleRu: "Контекст безопасности",
    summary: "Container/pod security options (user, capabilities, privilege controls).",
    summaryRu: "Параметры безопасности контейнера/pod (пользователь, capabilities, привилегии).",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#security-context",
    docsLinkRu: "docs/k8s-fields-guide.md#security-context",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/configure-pod-container/security-context/",
    example: "securityContext: |-\n  runAsNonRoot: true\n  runAsUser: 1000\n  readOnlyRootFilesystem: true\n",
  },
  affinity: {
    title: "Affinity",
    titleRu: "Affinity",
    summary: "Rules for pod placement by labels/topology.",
    summaryRu: "Правила размещения pod по лейблам и топологии.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#affinity-tolerations-nodeselector",
    docsLinkRu: "docs/k8s-fields-guide.md#affinity-tolerations-nodeselector",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/scheduling-eviction/assign-pod-node/",
    example: "affinity: |-\n  podAntiAffinity:\n    preferredDuringSchedulingIgnoredDuringExecution:\n      - weight: 100\n",
  },
  tolerations: {
    title: "Tolerations",
    titleRu: "Tolerations",
    summary: "Allows scheduling on tainted nodes matching toleration rules.",
    summaryRu: "Позволяет запуск на tainted-нодах при совпадении правил toleration.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#affinity-tolerations-nodeselector",
    docsLinkRu: "docs/k8s-fields-guide.md#affinity-tolerations-nodeselector",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/scheduling-eviction/taint-and-toleration/",
    example: "tolerations: |-\n  - key: dedicated\n    operator: Equal\n    value: workload\n    effect: NoSchedule\n",
  },
  nodeSelector: {
    title: "Node Selector",
    titleRu: "Node Selector",
    summary: "Simple label-based node selection for pods.",
    summaryRu: "Простой выбор нод для pod по label-ключам.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#affinity-tolerations-nodeselector",
    docsLinkRu: "docs/k8s-fields-guide.md#affinity-tolerations-nodeselector",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/scheduling-eviction/assign-pod-node/",
    example: "nodeSelector: |-\n  kubernetes.io/os: linux\n",
  },
  volumes: {
    title: "Volumes",
    titleRu: "Volumes",
    summary: "Pod volume declarations attached to containers via `volumeMounts`.",
    summaryRu: "Описание томов pod, подключаемых в контейнеры через `volumeMounts`.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#volumes-and-volumemounts",
    docsLinkRu: "docs/k8s-fields-guide.md#volumes-and-volumemounts",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/storage/volumes/",
    example: "volumes: |-\n  - name: cache\n    emptyDir: {}\n",
  },
  volumeMounts: {
    title: "Volume Mounts",
    titleRu: "Volume Mounts",
    summary: "Container mount points for declared volumes.",
    summaryRu: "Точки монтирования томов в контейнере.",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#volumes-and-volumemounts",
    docsLinkRu: "docs/k8s-fields-guide.md#volumes-and-volumemounts",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/storage/volumes/",
    example: "volumeMounts: |-\n  - name: cache\n    mountPath: /tmp/cache\n",
  },
  lifecycle: {
    title: "Lifecycle Hooks",
    titleRu: "Lifecycle Hooks",
    summary: "Container startup/shutdown hooks (`postStart`, `preStop`).",
    summaryRu: "Хуки старта/остановки контейнера (`postStart`, `preStop`).",
    type: "YAML block string | map",
    docsLinkEn: "docs/k8s-fields-guide.en.md#lifecycle-hooks",
    docsLinkRu: "docs/k8s-fields-guide.md#lifecycle-hooks",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/containers/container-lifecycle-hooks/",
    example: "lifecycle: |-\n  preStop:\n    exec:\n      command: [\"/bin/sh\", \"-c\", \"sleep 5\"]\n",
  },
  envFrom: {
    title: "envFrom",
    summary: "Imports env variables from ConfigMap/Secret references.",
    type: "YAML block string | list",
    docsLinkEn: "docs/k8s-fields-guide.en.md#envfrom",
    docsLinkRu: "docs/k8s-fields-guide.md#envfrom",
    k8sDocsLink: "https://kubernetes.io/docs/tasks/inject-data-application/define-environment-variable-container/",
    example: "envFrom: |-\n  - secretRef:\n      name: app-secrets\n  - configMapRef:\n      name: app-config\n",
  },
  ports: {
    title: "Ports",
    summary: "Port declarations for containers/services.",
    type: "YAML block string | list",
    docsLinkEn: "docs/k8s-fields-guide.en.md#ports",
    docsLinkRu: "docs/k8s-fields-guide.md#ports",
    k8sDocsLink: "https://kubernetes.io/docs/concepts/services-networking/service/",
    example: "ports: |-\n  - name: http\n    containerPort: 8080\n",
  },
};

export function findKeyPathAtPosition(text: string, line: number, character: number): string[] | null {
  const lines = text.split(/\r?\n/);
  if (line < 0 || line >= lines.length) {
    return null;
  }

  const stack: Array<{ indent: number; key: string }> = [];
  let blockScalarIndent: number | null = null;

  for (let i = 0; i <= line; i += 1) {
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

    const keyMatch = raw.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!keyMatch) {
      continue;
    }

    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }

    const key = keyMatch[2];
    const keyStart = keyMatch[1].length;
    const keyEnd = keyStart + key.length;
    const path = [...stack.map((s) => s.key), key];

    if (i === line) {
      // Show field hover for the whole key-value line, not only exact key token.
      // This avoids falling back to generic schema hover when user points at ":" or value area.
      if (character >= keyStart && character <= raw.length) {
        return path;
      }
    }

    stack.push({ indent, key });
    if (/^[|>][-+]?\s*$/.test(keyMatch[3].trim())) {
      blockScalarIndent = indent;
    }
  }

  return null;
}

export function findFieldDoc(path: string[]): FieldDoc | null {
  for (const rule of RULES) {
    if (matchesPath(rule.pattern, path)) {
      return rule.doc;
    }
  }
  const byLastKey = LAST_KEY_RULES[path[path.length - 1]];
  if (byLastKey) {
    return byLastKey;
  }
  const dynamic = dynamicFieldDoc(path);
  if (dynamic) {
    return dynamic;
  }
  const schemaDoc = schemaFieldDoc(path);
  if (schemaDoc) {
    return schemaDoc;
  }
  const unknown = unknownFieldDoc(path);
  if (unknown) {
    return unknown;
  }
  return null;
}

export function buildFieldDocMarkdown(path: string[], doc: FieldDoc): string {
  return buildFieldDocMarkdownLocalized(path, doc, "en");
}

export function buildFieldDocMarkdownLocalized(path: string[], doc: FieldDoc, locale: string): string {
  const ru = isRussianLocale(locale);
  const title = ru ? (doc.titleRu ?? doc.title) : doc.title;
  const summary = ru ? (doc.summaryRu ?? doc.summary) : doc.summary;
  const notes = ru ? (doc.notesRu ?? doc.notes) : doc.notes;
  const typeText = ru ? (doc.typeRu ?? doc.type) : doc.type;

  const parts: string[] = [];
  parts.push(`**${title}**`);
  parts.push(`\`${path.join(".")}\``);
  parts.push("");
  parts.push(summary.trim());
  parts.push("");
  parts.push(`**${ru ? "Тип" : "Type"}**: \`${typeText}\``);
  if (notes && notes.length > 0) {
    parts.push("");
    parts.push(`**${ru ? "Примечания" : "Notes"}**:`);
    for (const n of notes) {
      parts.push(`- ${n}`);
    }
  }
  if (doc.example) {
    parts.push("");
    parts.push(`**${ru ? "Пример" : "Example"}**:`);
    parts.push("```yaml");
    parts.push(doc.example.trimEnd());
    parts.push("```");
  }
  if (doc.docsLink || doc.docsLinkEn || doc.docsLinkRu) {
    parts.push("");
    const localLink = resolveDocsLink(doc, ru);
    const onlineLink = toOnlineDocsLink(localLink);
    parts.push(`**${ru ? "Документация" : "Docs"}**: [${ru ? "Открыть онлайн" : "Open Online"}](${onlineLink})`);
  }
  if (doc.k8sDocsLink) {
    parts.push("");
    parts.push(`**${ru ? "Kubernetes Docs" : "Kubernetes Docs"}**: [${doc.k8sDocsLink}](${doc.k8sDocsLink})`);
  }
  return parts.join("\n");
}

function matchesPath(pattern: string[], path: string[]): boolean {
  if (pattern.length !== path.length) {
    return false;
  }
  for (let i = 0; i < pattern.length; i += 1) {
    if (pattern[i] !== "*" && pattern[i] !== path[i]) {
      return false;
    }
  }
  return true;
}

function dynamicFieldDoc(path: string[]): FieldDoc | null {
  if (path.length === 1) {
    const group = path[0];
    if (group === "global") {
      return {
        title: "Global Settings",
        summary: "Common settings applied across groups: env selection, includes and deploy/release controls.",
        type: "map",
        notes: [
          "Most important keys: `env`, `_includes`, `deploy`, `releases`.",
        ],
        example: "global:\n  env: prod\n",
      };
    }
    if (BUILTIN_GROUPS.has(group)) {
  return {
    title: "Built-in Group",
    titleRu: "Встроенная группа",
    summary: `Top-level built-in group \`${group}\` rendered by helm-apps library templates.`,
    summaryRu: `Верхнеуровневая встроенная группа \`${group}\`, рендерится шаблонами библиотеки helm-apps.`,
    type: "map(appName -> appConfig)",
    notes: [
      "Each child key is an app name.",
      "Use `enabled`, `_include`, and renderer-specific fields inside app.",
    ],
    notesRu: [
      "Каждый дочерний ключ — это имя приложения.",
      "Используйте `enabled`, `_include` и поля выбранного рендерера внутри app.",
    ],
    example: `${group}:\n  my-app:\n    enabled: true\n`,
  };
    }
    return {
      title: "Custom Group",
      titleRu: "Кастомная группа",
      summary: "Top-level custom group. Renderer type is usually defined in `__GroupVars__.type`.",
      summaryRu: "Кастомная верхнеуровневая группа. Тип рендерера обычно задается в `__GroupVars__.type`.",
      type: "map",
      notes: [
        "Can map to any built-in renderer or custom `<type>.render` template.",
      ],
      notesRu: [
        "Может маппиться на любой встроенный рендерер или на custom-шаблон `<type>.render`.",
      ],
      example: `${group}:\n  __GroupVars__:\n    type: apps-stateless\n  my-app:\n    enabled: true\n`,
    };
  }

  if (path.length === 2 && path[0] !== "global" && path[1] !== "__GroupVars__") {
    return {
      title: "App Entry",
      titleRu: "Узел приложения",
      summary: `Application config \`${path[1]}\` inside group \`${path[0]}\`.`,
      summaryRu: `Конфигурация приложения \`${path[1]}\` внутри группы \`${path[0]}\`.`,
      type: "map",
      notes: [
        "Typical keys: `enabled`, `_include`, labels/annotations, spec fields of selected renderer.",
      ],
      notesRu: [
        "Типичные ключи: `enabled`, `_include`, labels/annotations, spec-поля выбранного рендерера.",
      ],
      example: `${path[0]}:\n  ${path[1]}:\n    enabled: true\n`,
    };
  }

  return null;
}

function schemaFieldDoc(path: string[]): FieldDoc | null {
  const root = loadSchemaRoot();
  if (!root) {
    return null;
  }
  const matched = resolveSchemaAtPath(root, path);
  if (!matched) {
    return null;
  }

  const notes: string[] = [];
  const notesRu: string[] = [];
  const schemaType = formatSchemaType(matched);
  if (matched.enum && matched.enum.length > 0) {
    const allowed = `${matched.enum.slice(0, 12).map((v) => `\`${String(v)}\``).join(", ")}${matched.enum.length > 12 ? ", ..." : ""}`;
    notes.push(`Allowed values: ${allowed}`);
    notesRu.push(`Допустимые значения: ${allowed}`);
  }
  if (matched.default !== undefined) {
    const def = `\`${stringifyValue(matched.default)}\``;
    notes.push(`Default: ${def}`);
    notesRu.push(`Значение по умолчанию: ${def}`);
  }

  const childHints = collectKnownChildHints(matched);
  if (childHints.length > 0) {
    notes.push(`Known child keys: ${childHints.join(", ")}`);
    notesRu.push(`Известные дочерние ключи: ${childHints.join(", ")}`);
  }

  const variantTypes = collectVariantTypes(matched);
  if (variantTypes.length > 0) {
    const variants = variantTypes.map((v) => `\`${v}\``).join(", ");
    notes.push(`Schema variants: ${variants}`);
    notesRu.push(`Варианты схемы: ${variants}`);
  }

  const description = matched.description?.trim() ?? "";
  const fallback = contextualSchemaSummary(path);
  return {
    title: `Schema Field: ${path[path.length - 1]}`,
    titleRu: `Поле схемы: ${path[path.length - 1]}`,
    summary: description.length > 0
      ? description
      : fallback.en,
    summaryRu: description.length > 0
      ? description
      : fallback.ru,
    type: schemaType,
    notes: notes.length > 0 ? notes : undefined,
    notesRu: notesRu.length > 0 ? notesRu : undefined,
  };
}

function unknownFieldDoc(path: string[]): FieldDoc | null {
  if (path.length < 3) {
    return null;
  }
  const group = path[0];
  const app = path[1];
  const key = path[path.length - 1];
  const root = loadSchemaRoot();
  const parentPath = path.slice(0, -1);
  const parentSchema = root ? resolveSchemaAtPath(root, parentPath) : null;
  const hints = parentSchema ? collectKnownChildHints(parentSchema) : [];

  const notes = [
    "This key is not yet documented by extension hover catalog.",
    "It can still be fully valid in library/custom renderer.",
    "Check rendered manifest diff to confirm this field affects output.",
  ];
  if (hints.length > 0) {
    notes.push(`Known sibling keys here: ${hints.join(", ")}`);
  }

  return {
    title: "Custom or Unknown Field",
    titleRu: "Кастомное или неизвестное поле",
    summary: `\`${key}\` under \`${group}.${app}\` is treated as custom payload or group-specific field.`,
    summaryRu: `\`${key}\` в \`${group}.${app}\` трактуется как кастомное поле или group-specific параметр.`,
    type: "custom",
    notes,
    notesRu: [
      "Этот ключ пока не каталогизирован в hover-справочнике расширения.",
      "Поле может быть полностью валидным для библиотеки/кастомного рендерера.",
      "Проверяйте итоговый рендер/дифф манифестов для подтверждения эффекта.",
      ...(hints.length > 0 ? [`Известные соседние ключи: ${hints.join(", ")}`] : []),
    ],
  };
}

function loadSchemaRoot(): JsonSchema | null {
  if (schemaRootCache) {
    return schemaRootCache;
  }
  try {
    const candidates = [
      path.resolve(__dirname, "../../schemas/values.schema.json"),
      path.resolve(__dirname, "../../../schemas/values.schema.json"),
    ];
    for (const schemaPath of candidates) {
      try {
        const raw = readFileSync(schemaPath, "utf8");
        schemaRootCache = JSON.parse(raw) as JsonSchema;
        return schemaRootCache;
      } catch {
        // try next candidate
      }
    }
    return null;
  } catch {
    return null;
  }
}

function resolveSchemaAtPath(root: JsonSchema, pathParts: string[]): JsonSchema | null {
  return walkSchema(root, pathParts, 0, root);
}

function walkSchema(current: JsonSchema, pathParts: string[], index: number, root: JsonSchema): JsonSchema | null {
  const schema = resolveRefs(current, root);
  if (!schema) {
    return null;
  }
  if (index >= pathParts.length) {
    return schema;
  }

  const seg = pathParts[index];
  const candidates = nextSchemasForSegment(schema, seg, root);
  for (const candidate of candidates) {
    const resolved = walkSchema(candidate, pathParts, index + 1, root);
    if (resolved) {
      return resolved;
    }
  }
  return null;
}

function nextSchemasForSegment(schema: JsonSchema, segment: string, root: JsonSchema): JsonSchema[] {
  const out: JsonSchema[] = [];
  for (const variant of schemaVariants(schema, root)) {
    if (variant.properties && variant.properties[segment]) {
      out.push(variant.properties[segment]);
    }
    if (variant.patternProperties) {
      for (const [pattern, child] of Object.entries(variant.patternProperties)) {
        try {
          const re = new RegExp(pattern);
          if (re.test(segment)) {
            out.push(child);
          }
        } catch {
          // ignore broken schema regex
        }
      }
    }
    if (variant.additionalProperties && typeof variant.additionalProperties === "object") {
      out.push(variant.additionalProperties);
    }
  }
  return out;
}

function schemaVariants(schema: JsonSchema, root: JsonSchema): JsonSchema[] {
  const base = resolveRefs(schema, root);
  if (!base) {
    return [];
  }
  const out: JsonSchema[] = [base];
  for (const arr of [base.allOf, base.anyOf, base.oneOf]) {
    if (!arr) {
      continue;
    }
    for (const item of arr) {
      const resolved = resolveRefs(item, root);
      if (resolved) {
        out.push(resolved);
      }
    }
  }
  return out;
}

function resolveRefs(schema: JsonSchema | undefined, root: JsonSchema): JsonSchema | null {
  if (!schema) {
    return null;
  }
  let current: JsonSchema | undefined = schema;
  const seen = new Set<string>();
  while (current && current.$ref) {
    const ref = current.$ref;
    if (!ref.startsWith("#/") || seen.has(ref)) {
      break;
    }
    seen.add(ref);
    current = getByPointer(root, ref);
  }
  return current ?? null;
}

function getByPointer(root: JsonSchema, ptr: string): JsonSchema | undefined {
  const chunks = ptr.slice(2).split("/").map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~"));
  let cur: unknown = root;
  for (const c of chunks) {
    if (!cur || typeof cur !== "object" || !(c in (cur as Record<string, unknown>))) {
      return undefined;
    }
    cur = (cur as Record<string, unknown>)[c];
  }
  return cur as JsonSchema;
}

function collectKnownChildHints(schema: JsonSchema): string[] {
  const out = new Set<string>();
  if (schema.properties) {
    for (const key of Object.keys(schema.properties)) {
      out.add(`\`${key}\``);
    }
  }
  if (schema.patternProperties) {
    for (const key of Object.keys(schema.patternProperties)) {
      out.add(`pattern:${key}`);
    }
  }
  return [...out].slice(0, 14);
}

function formatSchemaType(schema: JsonSchema): string {
  if (Array.isArray(schema.type)) {
    return schema.type.join(" | ");
  }
  if (typeof schema.type === "string") {
    return schema.type;
  }
  if (schema.oneOf || schema.anyOf) {
    const variants = collectVariantTypes(schema);
    return variants.length > 0 ? `oneOf(${variants.join(" | ")})` : "oneOf";
  }
  if (schema.allOf) {
    const variants = collectVariantTypes(schema);
    return variants.length > 0 ? `allOf(${variants.join(" & ")})` : "allOf";
  }
  if (schema.properties || schema.patternProperties) {
    return "object";
  }
  return "unknown";
}

function collectVariantTypes(schema: JsonSchema): string[] {
  const out = new Set<string>();
  for (const arr of [schema.oneOf, schema.anyOf, schema.allOf]) {
    if (!arr) {
      continue;
    }
    for (const item of arr) {
      if (typeof item.type === "string") {
        out.add(item.type);
      } else if (Array.isArray(item.type)) {
        for (const t of item.type) {
          out.add(t);
        }
      } else if (item.properties || item.patternProperties || item.additionalProperties) {
        out.add("object");
      }
    }
  }
  return [...out];
}

function contextualSchemaSummary(path: string[]): { en: string; ru: string } {
  const key = path[path.length - 1] ?? "";
  const parent = path.length >= 2 ? path[path.length - 2] : "";
  const scope = detectScope(path);
  if (isNamedEntry(key, parent)) {
    return {
      en: `Named entry \`${key}\` in \`${parent}\` (${scope.en}). Usually this is a user-defined map item key.`,
      ru: `Именованный элемент \`${key}\` в \`${parent}\` (${scope.ru}). Обычно это пользовательский ключ элемента map.`,
    };
  }
  if (parent === "containers" || parent === "initContainers") {
    return {
      en: `Container-level field at ${scope.en}.`,
      ru: `Поле контейнерного уровня на уровне ${scope.ru}.`,
    };
  }
  if (key === "initContainers" || parent === "initContainers") {
    return {
      en: `Init-container settings at ${scope.en}.`,
      ru: `Настройки init-container на уровне ${scope.ru}.`,
    };
  }
  return {
    en: `Schema-defined field at ${scope.en}.`,
    ru: `Поле, определенное схемой, на уровне ${scope.ru}.`,
  };
}

function detectScope(path: string[]): { en: string; ru: string } {
  if (path[0] === "global") {
    return { en: "global scope", ru: "global" };
  }
  if (path.length >= 2 && path[1] === "__GroupVars__") {
    return { en: "group vars scope", ru: "group vars" };
  }
  if (path.length >= 2) {
    return { en: `app scope \`${path[0]}.${path[1]}\``, ru: `приложения \`${path[0]}.${path[1]}\`` };
  }
  return { en: "top-level scope", ru: "верхнего уровня" };
}

function isNamedEntry(key: string, parent: string): boolean {
  if (key === "__GroupVars__" || parent.length === 0) {
    return false;
  }
  if (!/^[a-z0-9_.-]+$/i.test(key)) {
    return false;
  }
  return /-\d+$/.test(key) || /\d$/.test(key) || key.includes("-");
}

function stringifyValue(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function isRussianLocale(locale: string): boolean {
  return locale.toLowerCase().startsWith("ru");
}

function resolveDocsLink(doc: FieldDoc, ru: boolean): string {
  if (ru && doc.docsLinkRu) {
    return doc.docsLinkRu;
  }
  if (!ru && doc.docsLinkEn) {
    return doc.docsLinkEn;
  }
  return doc.docsLink ?? doc.docsLinkEn ?? doc.docsLinkRu ?? "";
}

function toOnlineDocsLink(link: string): string {
  if (link.startsWith("http://") || link.startsWith("https://")) {
    return link;
  }
  const normalized = link.startsWith("/") ? link.slice(1) : link;
  return `${DOCS_ONLINE_BASE}${normalized}`;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}
