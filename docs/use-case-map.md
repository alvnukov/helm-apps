# Helm Apps: Use-Case Map
<a id="top"></a>

Карта для быстрого выбора решения:
- что нужно сделать;
- какие параметры использовать;
- где взять рабочий пример;
- что проверить перед merge/release.

## Быстрая навигация

- [Старт docs](README.md)
- [Parameter Index](parameter-index.md)
- [Reference](reference-values.md)
- [Cookbook](cookbook.md)
- [Operations](operations.md)

## 1. Нужен обычный HTTP/API сервис

- Параметры: [containers](reference-values.md#param-containers), [service](reference-values.md#param-service)
- Пример: [Базовый HTTP API](cookbook.md#example-basic-api)
- Проверки: `helm lint`, `helm template ... --set global.env=<env>`

## 2. Нужен внешний доступ через Ingress + TLS

- Параметры: [ingress](reference-values.md#param-ingress), [global.env](reference-values.md#param-global-env)
- Пример: [API + Ingress + TLS](cookbook.md#example-ingress-tls)
- Проверки: backend service/port, ingress class, tls secret/certificate
- Ops: [Ingress 404/502](operations.md#46-ingress-есть-но-404502)

## 3. Нужен CronJob или Job

- Параметры: [containers](reference-values.md#param-containers), [global._includes/_include](reference-values.md#param-global-includes)
- Пример: [CronJob](cookbook.md#example-cronjob)
- Проверки: schedule, backoffLimit, restartPolicy, image tag

## 4. Нужны секреты в env

- Параметры: [secretEnvVars](reference-values.md#param-secretenvvars), [fromSecretsEnvVars](reference-values.md#param-fromsecretsenvvars)
- Примеры:
  - [secretEnvVars](cookbook.md#example-secretenvvars)
  - [fromSecretsEnvVars](cookbook.md#example-fromsecretsenvvars)
- Проверки: отсутствие plaintext в git, корректность ключей в Secret

## 5. Нужны файловые конфиги в контейнере

- Параметры: [configFiles](reference-values.md#param-configfiles), [configFilesYAML](reference-values.md#param-configfilesyaml)
- Примеры:
  - [configFiles](cookbook.md#example-configfiles)
  - [configFilesYAML](cookbook.md#example-configfilesyaml)
- Проверки: mountPath, формат content, итог рендера ConfigMap/Secret

## 6. Нужен HPA/VPA

- Параметры: [horizontalPodAutoscaler](reference-values.md#param-hpa), [hpa.metrics](reference-values.md#param-hpa-metrics), [verticalPodAutoscaler](reference-values.md#param-vpa)
- Пример: [HPA для API](cookbook.md#example-hpa)
- Проверки: min/max, metrics, updateMode, conflicts HPA vs VPA
- Ops: [HPA не скейлит](operations.md#47-hpa-не-скейлит), [VPA не влияет](operations.md#48-vpa-не-влияет-на-pods)

## 7. Нужен ServiceAccount и RBAC

- Параметры: [serviceAccount](reference-values.md#param-serviceaccount)
- Пример: [ServiceAccount + ClusterRole](cookbook.md#example-serviceaccount)
- Проверки: role rules, binding namespace, права на нужные API

## 8. Нужны разные значения для разных окружений

- Параметры: [global.env](reference-values.md#param-global-env), [_include](reference-values.md#param-include), [global._includes](reference-values.md#param-global-includes)
- Пример: [env recipe](cookbook.md#example-global-env)
- Проверки:
  - env задается через `global.env`;
  - нет конфликтных regex ключей;
  - финальный рендер проверен в каждом target env.

## 9. Нужно переиспользование и минимум дублирования

- Параметры: [global._includes](reference-values.md#param-global-includes), [_include](reference-values.md#param-include)
- Пример merge: [README merge section](../README.md#example-global-includes-merge)
- Проверки:
  - порядок include осознанный;
  - локальные overrides минимальны и понятны;
  - финальный рендер совпадает с ожиданием.

## 10. Быстрый pre-merge чеклист

1. Сверить параметры в [Parameter Index](parameter-index.md).
2. Прогнать `helm lint`.
3. Прогнать `helm template ... --set global.env=<env>`.
4. Проверить соответствующий раздел в [Operations](operations.md).

Навигация: [Наверх](#top)

