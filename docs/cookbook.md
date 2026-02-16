# Helm Apps Library Cookbook
<a id="top"></a>

Готовые рецепты для типовых сценариев.
Все примеры можно адаптировать под ваш `global._includes`.

Быстрая навигация:
- [Старт docs](README.md)
- [Parameter Index](parameter-index.md)
- [Reference](reference-values.md)

Оглавление (часто используемое):
- [1. Базовый HTTP API](#1-базовый-http-api-stateless)
- [2. API + Ingress + TLS](#2-api--ingress--tls)
- [4. CronJob](#4-cronjob)
- [6. Секреты через secretEnvVars](#6-секреты-через-secretenvvars)
- [6.1 Общие Secret через sharedEnvSecrets](#61-общие-secret-через-sharedenvsecrets)
- [6.2 Приоритет sharedEnvSecrets/envFrom/secretEnvVars](#62-приоритет-sharedenvsecretsenvfromsecretenvvars)
- [9. configFilesYAML](#9-yaml-конфиг-с-env-override-configfilesyaml)
- [10. HPA](#10-hpa-для-api)
- [11. ServiceAccount + ClusterRole](#11-serviceaccount--clusterrole)
- [20. Как использовать cookbook](#20-как-использовать-cookbook)

## 1. Базовый HTTP API (stateless)
<a id="example-basic-api"></a>

```yaml
apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    replicas:
      _default: 2
      production: 4
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
        ports: |
          - name: http
            containerPort: 8080
        envVars:
          APP_ENV:
            _default: dev
            production: production
        resources:
          requests:
            mcpu: 200
            memoryMb: 256
          limits:
            mcpu: 1000
            memoryMb: 1024
    service:
      enabled: true
      ports: |
        - name: http
          port: 80
          targetPort: 8080
```

Параметры: [containers](reference-values.md#param-containers), [service](reference-values.md#param-service), [envVars](reference-values.md#param-envvars)
Навигация: [Parameter Index](parameter-index.md#workload) | [Наверх](#top)

## 2. API + Ingress + TLS
<a id="example-ingress-tls"></a>

```yaml
apps-ingresses:
  api:
    _include: ["apps-ingresses-defaultIngress"]
    ingressClassName: nginx
    host: api.example.org
    paths: |
      - path: /
        pathType: Prefix
        backend:
          service:
            name: api
            port:
              number: 80
    tls:
      enabled: true
```

Параметры: [ingress](reference-values.md#param-ingress), [global.env](reference-values.md#param-global-env)
Навигация: [Parameter Index](parameter-index.md#networking-and-scaling) | [Наверх](#top)

## 3. Worker без Service

```yaml
apps-stateless:
  worker:
    _include: ["apps-stateless-defaultApp"]
    service:
      enabled: false
    containers:
      main:
        image:
          name: worker
          staticTag: "1.0.0"
        command: |
          - /app/worker
        envVars:
          QUEUE: default
```

## 4. CronJob
<a id="example-cronjob"></a>

```yaml
apps-cronjobs:
  sync-every-5m:
    _include: ["apps-cronjobs-defaultCronJob"]
    schedule: "*/5 * * * *"
    containers:
      main:
        image:
          name: sync
          staticTag: "2.1.0"
        command: |
          - /app/sync
        envVars:
          LOG_LEVEL: info
```

Параметры: [containers](reference-values.md#param-containers), [global._includes/_include](reference-values.md#param-global-includes)
Навигация: [Parameter Index](parameter-index.md#core) | [Наверх](#top)

## 5. One-shot Job (migration)

```yaml
apps-jobs:
  db-migrate:
    _include: ["apps-jobs-defaultJob"]
    backoffLimit: 1
    containers:
      main:
        image:
          name: migrate
          staticTag: "3.0.0"
        command: |
          - /app/migrate
```

## 6. Секреты через `secretEnvVars`
<a id="example-secretenvvars"></a>

```yaml
apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
        secretEnvVars:
          DB_PASSWORD: very-secret
          JWT_SECRET:
            _default: dev-secret
            production: prod-secret
```

Параметры: [secretEnvVars](reference-values.md#param-secretenvvars)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 6.1 Общие Secret через `sharedEnvSecrets`
<a id="example-sharedenvsecrets"></a>

```yaml
apps-secrets:
  common-runtime:
    envVars:
      LOG_FORMAT: json
      TZ: UTC

apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
        sharedEnvSecrets:
          - common-runtime
```

Что важно:
- `sharedEnvSecrets` задается списком;
- элементы списка должны быть строковыми именами Secret (или env-map со строками);
- список поддерживается только в `containers.*` / `initContainers.*`.

Параметры: [sharedEnvSecrets](reference-values.md#param-sharedenvsecrets), [apps-secrets](reference-values.md#param-apps-secrets)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 6.2 Приоритет `sharedEnvSecrets`/`envFrom`/`secretEnvVars`
<a id="example-sharedenvsecrets-priority"></a>

```yaml
apps-secrets:
  common-runtime:
    envVars:
      ORDER_KEY: from-shared
  manual-env:
    envVars:
      ORDER_KEY: from-manual

apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
        sharedEnvSecrets:
          - common-runtime
        envFrom: |
          - secretRef:
              name: "manual-env"
        secretEnvVars:
          ORDER_KEY: from-secret-env-vars
```

Порядок применения (низкий -> высокий приоритет):
- `sharedEnvSecrets`
- `envFrom`
- auto-secret из `secretEnvVars`

Старый контракт сохранен: без `sharedEnvSecrets` поведение `envFrom -> secretEnvVars` не меняется.

Параметры: [sharedEnvSecrets](reference-values.md#param-sharedenvsecrets), [secretEnvVars](reference-values.md#param-secretenvvars), [envFrom](reference-values.md#param-containers)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 7. Из внешнего Secret через `fromSecretsEnvVars`
<a id="example-fromsecretsenvvars"></a>

```yaml
apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
        fromSecretsEnvVars:
          external-secret:
            APP_DB_PASSWORD: db_password
            APP_API_TOKEN: api_token
```

Параметры: [fromSecretsEnvVars](reference-values.md#param-fromsecretsenvvars)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 8. Файлы конфигурации (ConfigMap mount)
<a id="example-configfiles"></a>

```yaml
apps-stateless:
  nginx:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: nginx
          staticTag: "1.27"
        configFiles:
          nginx.conf:
            mountPath: /etc/nginx/nginx.conf
            content: |
              worker_processes auto;
              events { worker_connections 1024; }
```

Параметры: [configFiles](reference-values.md#param-configfiles)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 9. YAML-конфиг с env override (`configFilesYAML`)
<a id="example-configfilesyaml"></a>

```yaml
apps-stateless:
  app:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: app
          staticTag: "1.0.0"
        configFilesYAML:
          app.yaml:
            mountPath: /etc/app/app.yaml
            content:
              db:
                host:
                  _default: db.dev
                  production: db.prod
              cache:
                ttlSeconds:
                  _default: 30
                  production: 300
```

Параметры: [configFilesYAML](reference-values.md#param-configfilesyaml), [global.env](reference-values.md#param-global-env)
Навигация: [Parameter Index](parameter-index.md#containers-envconfig) | [Наверх](#top)

## 10. HPA для API
<a id="example-hpa"></a>

```yaml
apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: api
          staticTag: "1.0.0"
    horizontalPodAutoscaler:
      enabled: true
      minReplicas: 2
      maxReplicas: 10
      behavior: |
        scaleDown:
          policies:
            - type: Percent
              value: 10
              periodSeconds: 60
      metrics:
        cpu:
          enabled: true
          averageUtilization: 70
        memory:
          enabled: true
          averageUtilization: 80
```

Параметры: [horizontalPodAutoscaler](reference-values.md#param-hpa), [hpa.metrics](reference-values.md#param-hpa-metrics)
Навигация: [Parameter Index](parameter-index.md#networking-and-scaling) | [Наверх](#top)

## 11. ServiceAccount + ClusterRole
<a id="example-serviceaccount"></a>

```yaml
apps-stateless:
  metrics-client:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: client
          staticTag: "1.0.0"
    serviceAccount:
      enabled: true
      name: metrics-client
      clusterRole:
        name: metrics-client:reader
        rules: |
          - apiGroups: ["monitoring.coreos.com"]
            resources: ["prometheuses/http"]
            resourceNames: ["main", "longterm"]
            verbs: ["get"]
```

Параметры: [serviceAccount](reference-values.md#param-serviceaccount)
Навигация: [Parameter Index](parameter-index.md#workload) | [Наверх](#top)

## 12. Stateful сервис с PVC
<a id="example-stateful-pvc"></a>

```yaml
apps-stateful:
  redis:
    _include: ["apps-stateful-defaultApp"]
    replicas: 1
    containers:
      main:
        image:
          name: redis
          staticTag: "7.2"
        ports: |
          - name: redis
            containerPort: 6379
        persistantVolumes:
          data:
            mountPath: /data
            size:
              _default: 1Gi
              production: 20Gi
            storageClass: fast-ssd
```

## 13. Dedicated ConfigMap/Secret resources

```yaml
apps-configmaps:
  shared-env:
    _include: ["apps-configmaps-defaultConfigmap"]
    envVars:
      FEATURE_FLAG_X: "true"
      REQUEST_TIMEOUT_MS:
        _default: "1000"
        production: "5000"

apps-secrets:
  shared-secret:
    _include: ["apps-secrets-defaultSecret"]
    envVars:
      API_KEY: secret
```

## 14. Внешний Service через `apps-services`

```yaml
apps-services:
  api-internal:
    _include: ["apps-defaults"]
    ports: |
      - name: http
        port: 80
        targetPort: 8080
    selector: |
      app: api
```

## 15. Пользовательская группа и mix app types

```yaml
payment:
  __GroupVars__:
    type: apps-stateless
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: payment-api
          staticTag: "1.0.0"
  ingress:
    __AppType__: apps-ingresses
    _include: ["apps-ingresses-defaultIngress"]
    host: pay.example.org
    paths: |
      - path: /
        pathType: Prefix
        backend:
          service:
            name: api
            port:
              number: 80
```

## 16. Рецепт с `_default` + regex env
<a id="example-global-env"></a>

```yaml
apps-stateless:
  env-aware:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: app
          staticTag: "1.0.0"
        envVars:
          LOG_LEVEL:
            _default: info
            production: warning
            "^prod-.*$": error
          FEATURE_ALPHA:
            _default: "false"
            "^dev-.*$": "true"
```

Параметры: [global.env](reference-values.md#param-global-env), [envVars](reference-values.md#param-envvars)
Навигация: [Parameter Index](parameter-index.md#core) | [Наверх](#top)

## 17. apps-infra: NodeUser

```yaml
apps-infra:
  node-users:
    platform-admin:
      enabled: true
      uid: 2001
      isSudoer: true
      sshPublicKeys: |
        - ssh-rsa AAAAB3Nza...
      extraGroups: |
        - wheel
      nodeGroups: |
        - worker
```

## 18. apps-dex-authenticators

```yaml
apps-dex-authenticators:
  auth-api:
    enabled: true
    applicationDomain: api.example.org
    applicationIngressClassName: nginx
    applicationIngressCertificateSecretName: api-example-org-tls
    allowedGroups: |
      - platform-admins
      - backend-team
```

## 19. apps-custom-prometheus-rules

```yaml
apps-custom-prometheus-rules:
  api-rules:
    groups:
      api-group:
        alerts:
          high-error-rate:
            isTemplate: false
            content: |
              expr: sum(rate(http_requests_total{status=~"5.."}[5m])) > 10
              for: 10m
              labels:
                severity_level: "3"
```

## 20. Как использовать cookbook

1. Выберите сценарий, близкий вашему сервису.
2. Скопируйте блок в `values.yaml`.
3. Подключите ваш include-профиль.
4. Добавьте env-overrides.
5. Прогоните `helm template` с нужным окружением через `global.env`.

Связанные документы:
- [docs/library-guide.md](library-guide.md)
- [docs/reference-values.md](reference-values.md)
- [docs/parameter-index.md](parameter-index.md)
- [tests/.helm/values.yaml](../tests/.helm/values.yaml)

Навигация: [Наверх](#top)
