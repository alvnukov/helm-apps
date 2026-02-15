# Helm Apps Library Handbook

## 1. Для кого этот документ

Документ предназначен для:
- разработчиков, которые деплоят приложения в Kubernetes через Helm;
- DevOps/SRE, которые поддерживают единый стандарт деплоя сервисов;
- ревьюеров конфигураций `.helm/values.yaml`.

Примечание: библиотека полностью поддерживает `Helm` и совместима с `werf`.  
Практически `werf` нередко удобнее в командной эксплуатации: меньше ручной склейки шагов между рендером и поставкой.  
Но все возможности библиотеки остаются полностью доступными через `Helm`.

Если нужен быстрый старт, сначала прочитайте раздел `3`.
Если нужен маршрут по документам, откройте `docs/README.md`.
Если нужен полный справочник полей, смотрите `docs/reference-values.md`.
Если нужны готовые шаблоны под типовые сценарии, смотрите `docs/cookbook.md`.

## 2. Что такое helm-apps и зачем его использовать

`helm-apps` это библиотечный Helm chart (`type: library`), который рендерит Kubernetes-ресурсы на основе унифицированной структуры `values.yaml`.

Ключевая идея:
- логика рендера ресурсов живет в одной библиотеке;
- сервисные репозитории описывают только конфигурацию;
- дефолты и переиспользование реализуются через `_include` и `global._includes`.

Почему это выгодно команде:
- меньше ручных манифестов и копипаста;
- единый формат деплоя во всех сервисах;
- проще ревью и онбординг;
- меньше расхождений между сервисами в runtime-поведении;
- быстрее массовые изменения платформенных практик.

Когда библиотека особенно полезна:
- десятки+ сервисов с похожими паттернами деплоя;
- необходимость стандартизировать логику HPA/VPA/PDB/Ingress/ServiceAccount;
- мультиокружения с разными параметрами в одном `values.yaml`.

## 3. Quick Start

1. Подключить библиотеку в `.helm/Chart.yaml` как dependency.
2. Создать шаблон инициализации:

```yaml
{{- include "apps-utils.init-library" $ }}
```

3. Задать в `global._includes` дефолтные профили.
4. Описать приложения в секциях `apps-*` или custom-группах.
5. Проверить рендер:

```bash
helm dependency update .helm
helm template my-app .helm --set global.env=prod
```

## 4. Базовая модель конфигурации

Верхний уровень `values.yaml`:
- `global` — общие переменные и include-блоки;
- `apps-*` — встроенные группы ресурсов;
- произвольные группы через `__GroupVars__`.

### 4.1 Встроенные группы `apps-*`

Библиотека поддерживает:
- `apps-stateless` (`Deployment`);
- `apps-stateful` (`StatefulSet`);
- `apps-jobs` (`Job`);
- `apps-cronjobs` (`CronJob`);
- `apps-ingresses` (`Ingress`, optional `Certificate`, optional `DexAuthenticator`);
- `apps-services` (`Service`);
- `apps-configmaps` (`ConfigMap`);
- `apps-secrets` (`Secret`);
- `apps-pvcs` (`PersistentVolumeClaim`);
- `apps-certificates` (`Certificate`);
- `apps-limit-range` (`LimitRange`);
- `apps-dex-clients` (`DexClient`);
- `apps-dex-authenticators` (`DexAuthenticator`);
- `apps-custom-prometheus-rules` (`CustomPrometheusRules`);
- `apps-grafana-dashboards` (`GrafanaDashboardDefinition`);
- `apps-kafka-strimzi` (Kafka + KafkaTopic + VPA под Strimzi);
- `apps-infra` (`NodeUser` и `NodeGroup` Deckhouse).

### 4.2 Произвольные группы через `__GroupVars__`

Позволяют описывать “логические” группы приложений:

```yaml
payment-group:
  __GroupVars__:
    type: apps-stateless
  api:
    _include: ["apps-stateless-defaultApp"]
  worker:
    _include: ["apps-stateless-defaultApp"]
```

Для отдельного приложения можно переопределить тип:

```yaml
payment-group:
  __GroupVars__:
    type: apps-stateless
  edge:
    __AppType__: apps-ingresses
```

## 5. Переиспользование конфигурации

### 5.1 `global._includes` + `_include`

`global._includes` хранит шаблонные блоки:

```yaml
global:
  _includes:
    apps-stateless-defaultApp:
      replicas: 2
      service:
        enabled: false
```

Подключение в приложении:

```yaml
apps-stateless:
  billing-api:
    _include: ["apps-stateless-defaultApp"]
```

Несколько include:

```yaml
_include: ["profile-base", "profile-prod", "profile-api"]
```

Практика:
- делите include на небольшие “профили”;
- используйте явные имена (`apps-stateless-defaultApp`, `profile-worker`);
- локальные overrides держите прямо в приложении.

### 5.2 `_include_from_file`

Поддерживается загрузка include-блоков из файлов через `global._includes`.
Используйте это для больших наборов дефолтов, чтобы не раздувать основной `values.yaml`.

## 6. Окружения: `_default`, env-override, regex

Любое значение может задаваться:
- как обычный скаляр;
- как map по окружениям.

Пример:

```yaml
replicas:
  _default: 2
  production: 5
  "^prod-.*$": 4
```

Алгоритм выбора значения:
1. точное совпадение `global.env`;
2. regex-совпадение по ключам;
3. `_default`.

Важно:
- несколько regex-совпадений для одного поля вызывают ошибку;
- для вложенных env-структур (`envYAML`, `configFilesYAML`) ожидается `_default`, иначе узел может быть проигнорирован логикой рендера.

## 7. Типы полей: “строка YAML” vs map/list

В библиотеке есть поля, которые вставляются в манифест как raw YAML.
Часто это делается через block string:

```yaml
annotations: |
  key: value
ports: |
  - name: http
    port: 80
```

Преимущество:
- 1:1 перенос kubernetes-структуры без дополнительной обвязки.

Риск:
- если передать не тот тип, можно получить неочевидный runtime-результат.

Рекомендация:
- держите schema-валидацию включенной;
- используйте рабочие шаблоны из `docs/cookbook.md`.

## 8. Контейнерный слой

`containers` и `initContainers` поддерживают:
- image: `name`, `staticTag`, `generateSignatureBasedTag`;
- process: `command`, `args`, `workingDir`;
- env: `envVars`, `secretEnvVars`, `envFrom`, `envYAML`, `fromSecretsEnvVars`;
- resources: `requests/limits` (`mcpu`, `memoryMb`, `ephemeralStorageMb`);
- configs: `configFiles`, `configFilesYAML`, `secretConfigFiles`;
- probes/lifecycle/security: `livenessProbe`, `readinessProbe`, `startupProbe`, `lifecycle`, `securityContext`;
- volumes: `volumeMounts`, `persistantVolumes`.

Особенности:
- `secretEnvVars` автоматически создают Secret и подключают его в `envFrom`;
- `configFiles*` автоматически создают ConfigMap/Secret и монтируются в контейнер;
- `alwaysRestart` добавляет псевдослучайный env `FL_APP_ALWAYS_RESTART`.

## 9. Слой Pod/Workload

Для `apps-stateless` и `apps-stateful` доступны:
- `replicas`;
- `affinity`, `tolerations`, `nodeSelector`, `topologySpreadConstraints`;
- `imagePullSecrets`, `volumes`;
- `serviceAccount` с optional `clusterRole`;
- `podDisruptionBudget`;
- `verticalPodAutoscaler`;
- `horizontalPodAutoscaler` (для stateless);
- `service`.

Для `apps-jobs`/`apps-cronjobs`:
- `backoffLimit`, `activeDeadlineSeconds`, `restartPolicy`;
- для cron: `schedule`, `concurrencyPolicy`, `startingDeadlineSeconds`, `successfulJobsHistoryLimit`, `failedJobsHistoryLimit`.

## 10. Сетевой слой

### 10.1 Service

`apps-services` или вложенный `service` у workload:
- `ports`;
- `selector`;
- `type`, `clusterIP`, `sessionAffinity`, и другие параметры Service API.

### 10.2 Ingress

`apps-ingresses`:
- `host`, `paths`;
- `class` и/или `ingressClassName`;
- `tls.enabled`;
- optional `tls.secret_name`.

Если `tls.enabled=true` и `secret_name` не задан:
- библиотека генерирует `Certificate` автоматически.

`dexAuth` в ingress:
- включает генерацию связанного `DexAuthenticator` для защиты приложения.

## 11. Безопасность и доступы

### 11.1 Secrets

Сценарии:
- `apps-secrets` для отдельного Secret ресурса;
- `secretEnvVars` в контейнере для привязки секретов к pod;
- `secretConfigFiles` для файловых секретов.

### 11.2 ServiceAccount и RBAC

В приложении:

```yaml
serviceAccount:
  enabled: true
  name: app-sa
  clusterRole:
    name: app-sa:read
    rules: |
      - apiGroups: [""]
        resources: ["pods"]
        verbs: ["get", "list"]
```

Библиотека создаст:
- `ServiceAccount`;
- `ClusterRole`;
- `ClusterRoleBinding`.

## 12. Масштабирование и SLO

### 12.1 VerticalPodAutoscaler

Поддерживается для workload-ресурсов и части специализированных групп.
Используйте:
- `updateMode: Off` для сбора метрик;
- `updateMode: Initial/Auto` по стратегии команды.

### 12.2 HorizontalPodAutoscaler

Поддержка:
- `cpu`, `memory`;
- object/custom metrics;
- optional генерация Deckhouse metric ресурсов (`customMetricResources`).

## 13. Observability

Поддерживаются:
- `apps-custom-prometheus-rules`;
- `apps-grafana-dashboards`;
- `deckhouseMetrics` в приложении.

Это позволяет держать метрики/правила алертинга рядом с конфигурацией деплоя сервиса.

## 14. Специализированные группы

### 14.1 `apps-kafka-strimzi`

Шаблоны для Strimzi:
- `Kafka` cluster;
- `KafkaTopic`;
- сопутствующие VPA.

Типичный сценарий:
- единый блок конфигурации Kafka в values;
- env-override для `prod/non-prod`.

### 14.2 `apps-infra`

Deckhouse-инфраструктурные сущности:
- `node-users`;
- `node-groups`.

Используйте для инфраструктурных репозиториев или platform-слоя.

## 15. Хуки и расширяемость

Поддерживаются pre-render hooks:
- group-level: `__GroupVars__._preRenderGroupHook`;
- app-level default: `__GroupVars__._preRenderAppHook`;
- app-level explicit: `_preRenderHook`.

Практические применения:
- массовая модификация группы перед рендером;
- автоматическое включение/клонирование приложений;
- вычисление derived-конфигурации.

## 16. Рекомендуемые практики команды

1. Выделяйте платформенные include-профили в `global._includes`.
2. Делайте значения окружений через `_default` + target-env overrides.
3. Не храните business-логику в Helm-шаблонах сервисов.
4. Для сложных структур используйте raw YAML блоки и шаблоны из cookbook.
5. Прогоняйте `helm template` в CI на каждом merge request.
6. Проверяйте значения schema-валидатором.

## 17. Антипаттерны

1. Копировать готовые Deployment/Service шаблоны между сервисами.
2. Размазывать дефолты по множеству несвязанных include-блоков.
3. Использовать “неявные” regex для env, создающие неоднозначности.
4. Смешивать в одном приложении слишком много unrelated-ролей (api+worker+cron).
5. Отключать schema-валидацию в CI.

## 18. Валидация

Основные артефакты:
- примеры: `tests/.helm/values.yaml`;
- схема: `tests/.helm/values.schema.json`.

Рекомендуемые проверки:

```bash
helm lint .helm
helm template my-app .helm --set global.env=prod
```

## 19. Миграция на библиотеку (пошагово)

1. Подключите библиотеку и инициализатор.
2. Перенесите один сервис в `apps-stateless`.
3. Вынесите общие настройки в include-профили.
4. Добавьте service/ingress/hpa/vpa/pdb.
5. Перенесите jobs/cronjobs.
6. Включите CI-проверки schema + render.
7. Только после стабилизации удаляйте legacy шаблоны.

## 20. Навигация по документации

- Концепция и архитектура: `docs/library-guide.md`
- Полный справочник полей: `docs/reference-values.md`
- Готовые рецепты: `docs/cookbook.md`
- Эксплуатация и troubleshooting: `docs/operations.md`
- Полные рабочие примеры: `tests/.helm/values.yaml`
- Схема валидации: `tests/.helm/values.schema.json`
