# Helm Apps Library: Reference по values

Документ описывает практический референс структуры `values.yaml`.
Он дополняет `docs/library-guide.md` и должен читаться вместе с ним.

## 1. Top-level ключи

Поддерживаемые секции:
- `global`
- `apps-stateless`
- `apps-stateful`
- `apps-jobs`
- `apps-cronjobs`
- `apps-services`
- `apps-ingresses`
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
- `werf`
- `helm-apps`

## 2. `global`

Типичные поля:
- `env`: текущее окружение (`dev`, `prod`, `production`, etc.);
- `_includes`: библиотека include-блоков;
- произвольные project-level переменные (`ci_url`, `baseUrl` и т.д.).

Пример:

```yaml
global:
  env: production
  ci_url: example.org
  _includes:
    apps-stateless-defaultApp:
      replicas:
        _default: 2
        production: 4
```

### 2.1 `global._includes` + `_include`: примеры merge

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

## 6. Env-паттерн

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

### 8.3 `secretConfigFiles`

```yaml
secretConfigFiles:
  token.txt:
    mountPath: /etc/secret/token.txt
    content: super-secret
```

## 9. Service block

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

## 10. Ingress block

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

## 11. Autoscaling blocks

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

`customMetricResources.<name>`:
- `enabled`
- `kind`
- `name` (optional)
- `query`

## 12. `podDisruptionBudget`

Поля:
- `enabled`
- `maxUnavailable`
- `minAvailable`

## 13. `serviceAccount`

Поля:
- `enabled`
- `name`
- `clusterRole`

`clusterRole`:
- `name`
- `rules`

## 14. Прочие `apps-*` секции

### 14.1 `apps-configmaps`

Поля app:
- `data`
- `binaryData`
- `envVars`

### 14.2 `apps-secrets`

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
- `type` (required)
- `enabled`
- `_include`
- `_preRenderGroupHook`
- `_preRenderAppHook`

## 16. Полезные ссылки

- Общая концепция: `docs/library-guide.md`
- Практические рецепты: `docs/cookbook.md`
- Рабочие примеры: `tests/.helm/values.yaml`
- JSON Schema: `tests/.helm/values.schema.json`
