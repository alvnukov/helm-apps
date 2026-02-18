# Helm Apps: Parameter Index

Быстрые переходы по параметрам: описание + рабочий пример.

Смежные документы:
- [Старт docs](README.md)
- [Quick Start](quickstart.md)
- [Decision Guide](decision-guide.md)
- [Reference](reference-values.md)
- [Cookbook](cookbook.md)
- [Use-case map](use-case-map.md)

## Core

| Параметр | Описание | Пример |
|---|---|---|
| `global.env` | [Описание](reference-values.md#param-global-env) | [Пример](cookbook.md#example-global-env) |
| `global._includes` | [Описание](reference-values.md#param-global-includes) | [Пример](../README.md#example-global-includes-merge) |
| `_include` | [Описание](reference-values.md#param-include) | [Пример](../README.md#example-include-concat) |
| `global.deploy` | [Описание](reference-values.md#param-global-deploy) | [Пример](reference-values.md#example-global-deploy) |
| `global.releases` | [Описание](reference-values.md#param-global-releases) | [Пример](reference-values.md#example-global-deploy) |
| `versionKey` | [Описание](reference-values.md#param-versionkey) | [Пример](reference-values.md#example-global-deploy) |

## Workload

| Параметр | Описание | Пример |
|---|---|---|
| `containers` | [Описание](reference-values.md#param-containers) | [Пример](cookbook.md#example-basic-api) |
| `service` | [Описание](reference-values.md#param-service) | [Пример](cookbook.md#example-basic-api) |
| `podDisruptionBudget` | [Описание](reference-values.md#param-pdb) | [Пример](../tests/.helm/values.yaml) |
| `serviceAccount` | [Описание](reference-values.md#param-serviceaccount) | [Пример](cookbook.md#example-serviceaccount) |

## Containers Env/Config

| Параметр | Описание | Пример |
|---|---|---|
| `envVars` | [Описание](reference-values.md#param-envvars) | [Пример](cookbook.md#example-basic-api) |
| `secretEnvVars` | [Описание](reference-values.md#param-secretenvvars) | [Пример](cookbook.md#example-secretenvvars) |
| `fromSecretsEnvVars` | [Описание](reference-values.md#param-fromsecretsenvvars) | [Пример](cookbook.md#example-fromsecretsenvvars) |
| `sharedEnvSecrets` | [Описание](reference-values.md#param-sharedenvsecrets) | [Пример](cookbook.md#example-sharedenvsecrets) |
| `sharedEnvConfigMaps` | [Описание](reference-values.md#param-sharedenvconfigmaps) | [Пример](cookbook.md#example-sharedenvconfigmaps) |
| `envFrom` | [Описание](reference-values.md#param-containers) | [Пример](cookbook.md#example-sharedenvsecrets-priority) |
| `envYAML` | [Описание](reference-values.md#param-envyaml) | [Пример](../tests/.helm/values.yaml) |
| `configFiles` | [Описание](reference-values.md#param-configfiles) | [Пример](cookbook.md#example-configfiles) |
| `configFilesYAML` | [Описание](reference-values.md#param-configfilesyaml) | [Пример](cookbook.md#example-configfilesyaml) |

## Networking and Scaling

| Параметр | Описание | Пример |
|---|---|---|
| `ingress` (`host/paths/tls`) | [Описание](reference-values.md#param-ingress) | [Пример](cookbook.md#example-ingress-tls) |
| `horizontalPodAutoscaler` | [Описание](reference-values.md#param-hpa) | [Пример](cookbook.md#example-hpa) |
| `horizontalPodAutoscaler.metrics` | [Описание](reference-values.md#param-hpa-metrics) | [Пример](cookbook.md#example-hpa) |
| `verticalPodAutoscaler` | [Описание](reference-values.md#param-vpa) | [Пример](../tests/.helm/values.yaml) |

## Resource-Specific Sections

| Параметр | Описание | Пример |
|---|---|---|
| `apps-configmaps` | [Описание](reference-values.md#param-apps-configmaps) | [Пример](cookbook.md#example-sharedenvconfigmaps) |
| `apps-secrets` | [Описание](reference-values.md#param-apps-secrets) | [Пример](cookbook.md#example-sharedenvsecrets) |
| `apps-network-policies` | [Описание](reference-values.md#param-apps-sections) | [Пример](../tests/.helm/values.yaml) |

## Extension and Reliability

| Параметр/Тема | Описание | Пример |
|---|---|---|
| `__GroupVars__.type` | [Описание](reference-values.md#param-custom-groups) | [Пример](cookbook.md#15-пользовательская-группа-и-mix-app-types) |
| `__AppType__` | [Описание](reference-values.md#param-custom-groups) | [Пример](cookbook.md#15-пользовательская-группа-и-mix-app-types) |
| `validation.strict` | [Описание](reference-values.md#2-global) | [Пример](../tests/.helm/values.yaml) |
| list-политика | [Cheat sheet](reference-values.md#param-cheat-sheet) | [FAQ](faq.md#2-почему-list-в-values-почти-везде-запрещены) |

## Дополнительно

- Вопросы и ответы: [faq.md](faq.md)
- Архитектура и приоритеты: [architecture.md](architecture.md)
- Эксплуатация: [operations.md](operations.md)
