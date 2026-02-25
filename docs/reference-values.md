# Helm Apps Library: Reference по values
<a id="top"></a>

Документ описывает практический референс структуры `values.yaml`.
Он дополняет `docs/library-guide.md` и должен читаться вместе с ним.

Быстрая навигация:
- [Старт docs](README.md)
- [Quick Start](quickstart.md)
- [Decision Guide](decision-guide.md)
- [Handbook](library-guide.md)
- [Cookbook](cookbook.md)
- [Parameter Index](parameter-index.md)

Оглавление:
- [1. Top-level ключи](#1-top-level-ключи)
- [2. global](#2-global)
- [5. containers / initContainers](#5-containers--initcontainers)
- [8. Config files](#8-config-files)
- [9. Service block](#9-service-block)
- [10. Ingress block](#10-ingress-block)
- [11. Autoscaling blocks](#11-autoscaling-blocks)
- [17. Cheat sheet](#param-cheat-sheet)

## 1. Top-level ключи

Поддерживаемые секции:
- `global`
- `apps-stateless`
- `apps-stateful`
- `apps-jobs`
- `apps-cronjobs`
- `apps-services`
- `apps-ingresses`
- `apps-network-policies`
- `apps-configmaps`
- `apps-secrets`
- `apps-pvcs`
- `apps-limit-range`
- `apps-certificates`
- `apps-dex-clients`
- `apps-dex-authenticators`
- `apps-custom-prometheus-rules`
- `apps-grafana-dashboards`
- `apps-kafka-strimzi`
- `apps-infra`
- произвольные custom-группы с `__GroupVars__`

Служебные ключи, которые могут появляться в merged values:
- `helm-apps`

## 2. `global`
<a id="param-global"></a>

Типичные поля:
- `env`: текущее окружение (`dev`, `prod`, `production`, etc.);
- `_includes`: библиотека include-блоков;
- `release`: декларативное управление версиями приложений;
- `validation.strict`: opt-in strict contract для проверки values;
- произвольные project-level переменные (`ci_url`, `baseUrl` и т.д.).

Пример:

```yaml
global:
  env: production
  ci_url: example.org
  validation:
    strict: false
  _includes:
    apps-stateless-defaultApp:
      replicas:
        _default: 2
        production: 4
```

Примечание по `validation.strict`:
- В ветке `1.x` значение по умолчанию — `false` (совместимость).
- Флаг добавлен как контракт для постепенного перехода к более строгой валидации без breaking changes.
- Текущая реализация strict-check сначала покрывает `apps-network-policies` (неизвестные ключи дают fail).
- На top-level strict-check валидирует только `apps-*` имена:
  - встроенные `apps-*` группы разрешены;
  - custom-группы разрешены через `__GroupVars__.type`;
  - неизвестная `apps-*` секция без `__GroupVars__` даёт fail.

### 2.1 `global.deploy` + `global.releases`
<a id="param-global-deploy"></a>
<a id="param-global-releases"></a>
<a id="example-global-deploy"></a>

`global.deploy` и `global.releases` включают режим декларативных релизов:
- `global.deploy.release`: имя релиза, выбираемое через env-map по `global.env`;
- `global.deploy.enabled`: автоматически включает app, если для него найдена версия;
- `global.deploy.annotateAllWithRelease`: если `true`, аннотация `helm-apps/release` ставится на все ресурсы текущего деплоя;
- `global.releases`: матрица `релиз -> appKey -> tag/version`.

Связанные app-параметры:
- `versionKey` — ключ приложения в `global.releases.<release>`.
  - параметр опционален;
  - если `versionKey` не задан, библиотека использует `app.name`.
<a id="param-versionkey"></a>

Пример:

```yaml
global:
  env: production
  deploy:
    enabled: true
    annotateAllWithRelease: false
    release:
      _default: production-v1
      production: production-v1
      dev: dev-v1
  releases:
    production-v1:
      release-web: "3.19"
    dev-v1:
      release-web: "3.19-dev"

apps-stateless:
  api:
    enabled: false
    versionKey: release-web
    containers:
      main:
        image:
          name: alpine
```

Поведение:
- библиотека выставляет `CurrentReleaseVersion` и `CurrentAppVersion` только для app, которые найдены в `global.releases.<release>`;
- `helm-apps/release` по умолчанию ставится только для app, найденных в release map;
- при `global.deploy.annotateAllWithRelease=true` `helm-apps/release` ставится всем ресурсам текущего деплоя;
- если `image.staticTag` не задан, используется `CurrentAppVersion`;
- если `CurrentAppVersion` тоже не задан, image резолвится через стандартный путь `Values.werf.image`;
- в metadata добавляются аннотации:
  - `helm-apps/release`
  - `helm-apps/app-version`
- при `global.deploy.enabled=true` app автоматически включается, когда версия найдена.

### 2.2 `global._includes` + `_include`: примеры merge
<a id="param-global-includes"></a>
<a id="param-include"></a>

Ниже примеры, как библиотека объединяет include-профили.

#### Пример A: Рекурсивный merge вложенных map

```yaml
global:
  _includes:
    base:
      service:
        enabled: true
        headless: false
    net:
      service:
        ports: |
          - name: http
            port: 80

apps-stateless:
  api:
    _include: ["base", "net"]
```

Итог:
- `service.enabled=true`
- `service.headless=false`
- `service.ports` добавлен из `net`

#### Пример B: Приоритет include по порядку

```yaml
global:
  _includes:
    base:
      replicas: 2
    prod:
      replicas: 5

apps-stateless:
  api:
    _include: ["base", "prod"]
```

Итог: `replicas=5`.

#### Пример C: Локальный override сильнее include

```yaml
global:
  _includes:
    base:
      replicas: 2

apps-stateless:
  api:
    _include: ["base"]
    replicas: 3
```

Итог: `replicas=3`.

#### Пример D: Env-map поведение при merge include

```yaml
global:
  _includes:
    base:
      replicas:
        _default: 2
        production: 4
    canary:
      replicas:
        _default: 1
        production: 2

apps-stateless:
  api:
    _include: ["base", "canary"]
```

Поведение в результате merge:
- ключ `production` будет взят из `base` (значение `4`);
- `_default` будет взят из `canary` (значение `1`).

Вывод: для env-map обязательно проверяйте финальный рендер в нужном окружении.

Навигация: [Parameter Index](parameter-index.md#core) | [Наверх](#top)

#### Пример E: `_include`-списки конкатенируются

```yaml
global:
  _includes:
    profile-a:
      _include: ["base-a"]
      replicas: 2
    profile-b:
      _include: ["base-b"]

apps-stateless:
  api:
    _include: ["profile-a", "profile-b"]
```

Итоговый include-chain для приложения объединяет `base-a` и `base-b`.

Важно:
- это поведение относится к служебному ключу `_include`;
- обычные списковые параметры библиотеки, как правило, задаются строковым YAML-блоком (`|`), поэтому их merge как native list обычно не применяется.

## 3. Общая форма приложения в `apps-*`

```yaml
apps-stateless:
  app-name:
    _include: ["profile-name"]
    enabled: true
    name: "custom-name"
    werfWeight: -10
    annotations: |
      key: value
    labels: |
      tier: backend
```

Общие поля, которые могут встречаться в большинстве app-типов:
- `_include`
- `enabled`
- `name`
- `werfWeight`
- `versionKey`
- `annotations`
- `labels`

## 4. Workload app-поля

Актуально для:
- `apps-stateless`
- `apps-stateful`
- `apps-jobs`
- `apps-cronjobs`

### 4.1 Pod/workload common

- `containers`
- `initContainers`
- `imagePullSecrets`
- `affinity`
- `tolerations`
- `nodeSelector`
- `volumes`
- `serviceAccount`
- `verticalPodAutoscaler`

### 4.2 Stateless/Stateful

Дополнительно:
- `replicas`
- `podDisruptionBudget`
- `service`
- `selector`
- `horizontalPodAutoscaler` (в основном для stateless)

Stateful-specific:
- `service.name` (для headless service),
- `updateStrategy`,
- `persistentVolumeClaimRetentionPolicy`,
- `volumeClaimTemplates`.

### 4.3 Jobs/CronJobs

Общие job-поля:
- `backoffLimit`
- `activeDeadlineSeconds`
- `restartPolicy`
- `ttlSecondsAfterFinished` (в соответствующем API-блоке)

Только cron:
- `schedule`
- `concurrencyPolicy`
- `startingDeadlineSeconds`
- `successfulJobsHistoryLimit`
- `failedJobsHistoryLimit`

## 5. `containers` / `initContainers`
<a id="param-containers"></a>

Форма:

```yaml
containers:
  main:
    enabled: true
    image:
      name: app
      staticTag: "1.0.0"
    command: |
      - /bin/app
    args: |
      - --serve
```

Поддерживаемые поля контейнера:
<a id="param-envvars"></a>
<a id="param-sharedenvconfigmaps"></a>
<a id="param-sharedenvsecrets"></a>
<a id="param-secretenvvars"></a>
<a id="param-fromsecretsenvvars"></a>
<a id="param-envyaml"></a>
- `enabled`
- `name`
- `image.name`
- `image.staticTag`
- `image.generateSignatureBasedTag`
- `command`
- `args`
- `envVars`
- `envYAML`
- `env`
- `sharedEnvConfigMaps` (список имён ConfigMap из `apps-configmaps`, подключается как `envFrom.configMapRef`)
- `sharedEnvSecrets` (список имён Secret из `apps-secrets`, подключается как `envFrom.secretRef`)
- `envFrom`
- `secretEnvVars`
- `fromSecretsEnvVars`
- `resources`
- `lifecycle`
- `livenessProbe`
- `readinessProbe`
- `startupProbe`
- `securityContext`
- `volumeMounts`
- `volumes`
- `ports`
- `configFiles`
- `configFilesYAML`
- `secretConfigFiles`
- `persistantVolumes`

### 5.1 `sharedEnvSecrets`

Назначение:
- подключить один или несколько общих Secret в env контейнера через `envFrom`;
- избежать дублирования `envFrom` между приложениями.

Формат:

```yaml
sharedEnvSecrets:
  - common-runtime
  - platform-observability
```

Типы значений:
- элемент списка: `string`;
- также поддерживается env-map со строковыми значениями (для env-aware выбора имени Secret).

Пример env-map:

```yaml
sharedEnvSecrets:
  - _default: common-runtime
    production: common-runtime-prod
```

Важно:
- `sharedEnvSecrets` указывается только на уровне контейнера (`containers.*` / `initContainers.*`);
- Secret может быть как из текущего релиза, так и внешним (из другого релиза/namespace), если имя известно.

### 5.2 `sharedEnvConfigMaps`

Назначение:
- подключить один или несколько общих ConfigMap в env контейнера через `envFrom`;
- избежать дублирования `envFrom` между приложениями.

Формат:

```yaml
sharedEnvConfigMaps:
  - common-runtime-cm
  - platform-observability-cm
```

Типы значений:
- элемент списка: `string`;
- также поддерживается env-map со строковыми значениями (для env-aware выбора имени ConfigMap).

Пример env-map:

```yaml
sharedEnvConfigMaps:
  - _default: common-runtime-cm
    production: common-runtime-cm-prod
```

Важно:
- `sharedEnvConfigMaps` указывается только на уровне контейнера (`containers.*` / `initContainers.*`);
- ConfigMap может быть как из текущего релиза, так и внешним (из другого релиза/namespace), если имя известно.

Порядок объединения `envFrom`-источников (низкий -> высокий приоритет):
- `sharedEnvConfigMaps`
- `sharedEnvSecrets`
- `envFrom`
- auto-secret из `secretEnvVars`

Это сохраняет прежнее поведение старых опций (`envFrom -> secretEnvVars`) и добавляет `sharedEnvConfigMaps`/`sharedEnvSecrets` как базовый слой.

Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 6. Env-паттерн
<a id="param-global-env"></a>

Любое поле, поддерживающее env-map:

```yaml
field:
  _default: value
  production: value2
  "^prod-.*$": value3
```

Используйте:
- `_default` для базового значения;
- явный env-ключ для таргет окружения;
- regex только когда реально нужен паттерн.
- окружение задавайте на стадии рендера/деплоя: `werf.env` или `global.env`.
- для автоматической маркировки ресурсов окружением включите `global.labels.addEnv: true`.

Если окружение не задано, рендер завершится ошибкой `E_ENV_REQUIRED`.

## 7. Ресурсы контейнера

Форма:

```yaml
resources:
  requests:
    mcpu: 100
    memoryMb: 256
    ephemeralStorageMb: 100
  limits:
    mcpu: 500
    memoryMb: 512
```

Поддержка env-map также применима к этим полям.

## 8. Config files
<a id="param-configfiles"></a>
<a id="param-configfilesyaml"></a>
<a id="param-secretconfigfiles"></a>

### 8.1 `configFiles`

```yaml
configFiles:
  app.yaml:
    mountPath: /etc/app/app.yaml
    content: |
      key: value
```

### 8.2 `configFilesYAML`

```yaml
configFilesYAML:
  app.yaml:
    mountPath: /etc/app/app.yaml
    content:
      key:
        _default: value
        production: prod-value
```

Контракт для list-значений в `content`:
- list считается атомарным значением (не мержится по индексам);
- env-map для list работает на уровне ключа (`exact env -> regex -> _default`);
- при `_include`-merge list наследуется только если ключ отсутствует в более приоритетном слое;
- если ключ уже определен в более приоритетном слое, list заменяется целиком (без конкатенации).

### 8.3 `secretConfigFiles`

```yaml
secretConfigFiles:
  token.txt:
    mountPath: /etc/secret/token.txt
    content: super-secret
```

Контракт:
- для каждого файла должен быть задан `content` (создать Secret в библиотеке) или `name` (смонтировать существующий Secret);
- при отсутствии обоих значений рендер падает с явной ошибкой конфигурации.

Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 9. Service block
<a id="param-service"></a>

Используется:
- как nested `service` у workload;
- как отдельный объект в `apps-services`.

Типовые поля:
- `enabled`
- `name`
- `ports`
- `selector`
- `type`
- `clusterIP`
- `sessionAffinity`
- `annotations`

Навигация: [Parameter Index](parameter-index.md#workload) | [Наверх](#top)

## 10. Ingress block
<a id="param-ingress"></a>

`apps-ingresses.<name>`:
- `class`
- `ingressClassName`
- `host`
- `paths`
- `annotations`
- `tls.enabled`
- `tls.secret_name`
- `dexAuth`

`dexAuth` поля:
- `enabled`
- `clusterDomain`

Навигация: [Parameter Index](parameter-index.md#networking-and-scaling) | [Наверх](#top)

## 11. Autoscaling blocks
<a id="param-vpa"></a>
<a id="param-hpa"></a>

### 11.1 `verticalPodAutoscaler`

- `enabled`
- `updateMode`
- `resourcePolicy`

### 11.2 `horizontalPodAutoscaler`

- `enabled`
- `minReplicas`
- `maxReplicas`
- `behavior`
- `metrics`
- `customMetricResources`
<a id="param-hpa-metrics"></a>

`customMetricResources.<name>`:
- `enabled`
- `kind`
- `name` (optional)
- `query`

Навигация: [Parameter Index](parameter-index.md#networking-and-scaling) | [Наверх](#top)

## 12. `podDisruptionBudget`
<a id="param-pdb"></a>

Поля:
- `enabled`
- `maxUnavailable`
- `minAvailable`

## 13. `serviceAccount`
<a id="param-serviceaccount"></a>

Поля:
- `enabled`
- `name`
- `clusterRole`

`clusterRole`:
- `name`
- `rules`

Навигация: [Parameter Index](parameter-index.md#workload) | [Наверх](#top)

<a id="param-apps-sections"></a>
## 14. Прочие `apps-*` секции

### 14.1 `apps-configmaps`
<a id="param-apps-configmaps"></a>

Поля app:
- `data`
- `binaryData`
- `envVars`

Часто используется как источник для `sharedEnvConfigMaps` в контейнерах.

### 14.2 `apps-secrets`
<a id="param-apps-secrets"></a>

Поля app:
- `type`
- `data`
- `envVars`

### 14.3 `apps-pvcs`

Поля app:
- `storageClassName`
- `accessModes`
- `resources`

### 14.4 `apps-limit-range`

Поля app:
- `limits`

### 14.5 `apps-certificates`

Поля app:
- `name` (optional override)
- `clusterIssuer`
- `host`
- `hosts`

### 14.6 `apps-dex-clients`

Поля app:
- `redirectURIs` (required для включенного ресурса)

### 14.7 `apps-dex-authenticators`

Поля app:
- `applicationDomain`
- `applicationIngressClassName`
- `applicationIngressCertificateSecretName`
- `allowedGroups`
- `sendAuthorizationHeader`
- `whitelistSourceRanges`
- `nodeSelector`
- `tolerations`

### 14.8 `apps-custom-prometheus-rules`

Поля app:
- `groups`

Глубже:
- `groups.<group>.alerts.<alert>.isTemplate`
- `groups.<group>.alerts.<alert>.content`

### 14.9 `apps-grafana-dashboards`

Поля app:
- `folder`

Dashboard definition читается из `dashboards/<name>.json`.

### 14.10 `apps-kafka-strimzi`

Поля app (основные):
- `kafka`
- `zookeeper`
- `entityOperator`
- `exporter`
- `topics`

Эта секция специализирована под Strimzi и обычно выносится в отдельный infra/service chart.

### 14.11 `apps-infra`

Содержит:
- `node-users`
- `node-groups`

`node-users.<name>`:
- `enabled`
- `uid` (required)
- `passwordHash`
- `sshPublicKey`
- `sshPublicKeys`
- `extraGroups`
- `nodeGroups`
- `isSudoer`
- `annotations`
- `labels`

<a id="param-custom-groups"></a>
## 15. Custom-группы

Форма:

```yaml
group-name:
  __GroupVars__:
    type: apps-stateless
    enabled: true
    _preRenderGroupHook: |
      {{/* hook */}}
    _preRenderAppHook: |
      {{/* hook */}}
  app-a:
    _include: ["apps-stateless-defaultApp"]
```

Важные поля `__GroupVars__`:
- `type` (required, может быть как строкой, так и env-map через `global.env`)
- `enabled`
- `_include`
- `_preRenderGroupHook`
- `_preRenderAppHook`

### 15.1 Custom renderer через `__GroupVars__.type`

`type` может указывать не только на встроенный `apps-*` рендерер, но и на пользовательский.

Контракт:
1. В values:
   - `__GroupVars__.type: my-custom-type`
2. В шаблонах chart приложения:
   - `define "my-custom-type.render"`
3. Библиотека передает стандартный контекст (`$`, `$.CurrentApp`, `$.CurrentGroupVars`, `$.Values`).

Важно: любые поля app из `group.<app>.*` доступны в custom renderer через `$.CurrentApp.*`.

Полный набор полезных переменных в custom renderer:
- `$` (root context),
- `$.Values`,
- `$.CurrentApp`,
- `$.CurrentGroupVars`,
- `$.CurrentGroup`,
- `$.CurrentPath`,
- `$.Release`,
- `$.Capabilities`,
- `$.Files`.

Пример с явным пробросом app-полей в `$.CurrentApp`:

```yaml
custom-services:
  __GroupVars__:
    type: custom-services
  service-a:
    enabled: true
    host:
      ip: service-a.example.local
      port: 8080
    extraLabels:
      app.kubernetes.io/part-of: platform
```

```yaml
{{- define "custom-services.render" -}}
{{- $ := . -}}
apiVersion: v1
kind: ConfigMap
metadata:
  name: {{ $.CurrentApp.name | quote }}
  labels:
    app.kubernetes.io/name: {{ $.CurrentApp.name | quote }}
    app.kubernetes.io/enabled: {{ printf "%v" $.CurrentApp.enabled | quote }}
{{- with $.CurrentApp.extraLabels }}
{{ toYaml . | indent 4 }}
{{- end }}
data:
  kind: "custom-services"
  host: {{ printf "%v:%v" $.CurrentApp.host.ip $.CurrentApp.host.port | quote }}
{{- end -}}
```

## 16. Полезные ссылки

- Общая концепция: [docs/library-guide.md](library-guide.md)
- Практические рецепты: [docs/cookbook.md](cookbook.md)
- Индекс параметров: [docs/parameter-index.md](parameter-index.md)
- Рабочие примеры: [tests/.helm/values.yaml](../tests/.helm/values.yaml)
- JSON Schema: [tests/.helm/values.schema.json](../tests/.helm/values.schema.json)

<a id="param-cheat-sheet"></a>
## 17. Тип поля -> поведение рендера (cheat sheet)

Ниже быстрый справочник по самым частым типам полей.

| Поле/группа | Ожидаемый тип в values | Как используется при рендере |
|---|---|---|
| `_include` | `array[string]` | Конкатенируется между include-профилями, затем применяется merge.
| `global.env` | `string` | Выбирает env-значение из map (`_default`, `production`, regex).
| `global.labels.addEnv` | `bool` или env-map bool | Если `true`, добавляет label `app.kubernetes.io/environment=<current env>` в metadata labels рендеримых сущностей.
| `replicas`, `enabled`, `werfWeight`, `priorityClassName` | scalar или env-map scalar | Резолвится через `fl.value` как скаляр.
| `envVars.<KEY>` / `secretEnvVars.<KEY>` | scalar или env-map scalar | Рендерится как env var value.
| `sharedEnvConfigMaps[]` | `string` или env-map string | Преобразуется в `envFrom.configMapRef.name` на уровне контейнера.
| `sharedEnvSecrets[]` | `string` или env-map string | Преобразуется в `envFrom.secretRef.name` на уровне контейнера.
| `command`, `args`, `ports`, `envFrom`, `affinity`, `tolerations`, `nodeSelector`, `volumes`, `paths`, `rules`, `resourcePolicy` | string или env-map string | Обычно передаются как YAML block string (`|`) и вставляются в манифест.
| `horizontalPodAutoscaler.metrics` | string или object | Поддерживает 2 режима: raw YAML строка или map-конфиг метрик.
| `configFiles.<name>.content` | string (обычно) | Контент ConfigMap/файла.
| `configFilesYAML.<name>.content` | object | Рекурсивно обрабатывается как YAML-дерево (с `_default` в узлах).
| `apps-*.<app>.data` / `binaryData` (ConfigMap/Secret) | string или object | Для ConfigMap/Secret может быть raw YAML string или map.

Практика:
- если поле описано как Kubernetes-блок, используйте YAML строку (`|`);
- native YAML list в values запрещены, кроме явно разрешенных путей (`_include`, `_include_files`, `*.containers.*.sharedEnvConfigMaps`, `*.initContainers.*.sharedEnvConfigMaps`, `*.containers.*.sharedEnvSecrets`, `*.initContainers.*.sharedEnvSecrets` и т.д.);
- для env-значений используйте scalar/env-map;
- итог всегда проверяйте через `helm template ... --set global.env=<env>`.

Навигация: [Parameter Index](parameter-index.md) | [Наверх](#top)
