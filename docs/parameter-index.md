# Helm Apps: Parameter Index

Быстрые переходы по параметрам: описание + рабочий пример.

## Core

| Параметр | Описание | Пример |
|---|---|---|
| `global.env` | [Описание](reference-values.md#param-global-env) | [Пример](cookbook.md#example-global-env) |
| `global._includes` | [Описание](reference-values.md#param-global-includes) | [Пример](../README.md#example-global-includes-merge) |
| `global.release` | [Описание](reference-values.md#param-global-release) | [Пример](reference-values.md#example-global-release) |
| `_include` | [Описание](reference-values.md#param-include) | [Пример](../README.md#example-include-concat) |

## Workload

| Параметр | Описание | Пример |
|---|---|---|
| `containers` | [Описание](reference-values.md#param-containers) | [Пример](cookbook.md#example-basic-api) |
| `service` | [Описание](reference-values.md#param-service) | [Пример](cookbook.md#example-basic-api) |
| `releaseKey` | [Описание](reference-values.md#param-releasekey) | [Пример](reference-values.md#example-global-release) |
| `podDisruptionBudget` | [Описание](reference-values.md#param-pdb) | [Пример](../tests/.helm/values.yaml) |
| `serviceAccount` | [Описание](reference-values.md#param-serviceaccount) | [Пример](cookbook.md#example-serviceaccount) |

## Containers Env/Config

| Параметр | Описание | Пример |
|---|---|---|
| `envVars` | [Описание](reference-values.md#param-envvars) | [Пример](cookbook.md#example-basic-api) |
| `sharedEnvSecrets` | [Описание](reference-values.md#param-containers) | [Пример](../tests/contracts/values.yaml) |
| `secretEnvVars` | [Описание](reference-values.md#param-secretenvvars) | [Пример](cookbook.md#example-secretenvvars) |
| `fromSecretsEnvVars` | [Описание](reference-values.md#param-fromsecretsenvvars) | [Пример](cookbook.md#example-fromsecretsenvvars) |
| `envYAML` | [Описание](reference-values.md#param-envyaml) | [Пример](../tests/.helm/values.yaml) |
| `configFiles` | [Описание](reference-values.md#param-configfiles) | [Пример](cookbook.md#example-configfiles) |
| `configFilesYAML` | [Описание](reference-values.md#param-configfilesyaml) | [Пример](cookbook.md#example-configfilesyaml) |

## Networking and Scaling

| Параметр | Описание | Пример |
|---|---|---|
| `ingress` (`host/paths/tls`) | [Описание](reference-values.md#param-ingress) | [Пример](cookbook.md#example-ingress-tls) |
| `verticalPodAutoscaler` | [Описание](reference-values.md#param-vpa) | [Пример](../tests/.helm/values.yaml) |
| `horizontalPodAutoscaler` | [Описание](reference-values.md#param-hpa) | [Пример](cookbook.md#example-hpa) |
| `horizontalPodAutoscaler.metrics` | [Описание](reference-values.md#param-hpa-metrics) | [Пример](cookbook.md#example-hpa) |

## Related Docs

- Общая концепция: [library-guide.md](library-guide.md)
- Полный референс: [reference-values.md](reference-values.md)
- Практические рецепты: [cookbook.md](cookbook.md)
- Операционная эксплуатация: [operations.md](operations.md)
