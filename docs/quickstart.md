# Helm Apps Quick Start (10 минут)
<a id="top"></a>

Цель: дать первый предсказуемый деплой через библиотеку за 10 минут, без погружения во все детали.

Быстрая навигация:
- [Старт docs](README.md)
- [Decision Guide](decision-guide.md)
- [Reference](reference-values.md)
- [Cookbook](cookbook.md)

## 1. Что нужно заранее

1. Helm 3.
2. Репозиторий приложения с `.helm/Chart.yaml`.
3. Доступ к кластеру Kubernetes (для `helm upgrade --install`) или локальная проверка `helm template`.

## 2. Подключить библиотеку

В `.helm/Chart.yaml`:

```yaml
apiVersion: v2
name: my-app
version: 1.0.0
dependencies:
  - name: helm-apps
    version: ~1
    repository: "@helm-apps"
```

Создать `.helm/templates/init-helm-apps-library.yaml`:

```yaml
{{- include "apps-utils.init-library" $ }}
```

Обновить зависимости:

```bash
helm repo add --force-update helm-apps https://alvnukov.github.io/helm-apps
helm dependency update .helm
```

## 3. Первый рабочий `values.yaml` (API + Service + Ingress)

```yaml
global:
  env: prod
  ci_url: example.org

apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    replicas:
      _default: 2
      prod: 3
    containers:
      main:
        image:
          name: nginx
          staticTag: "1.27.0"
        ports: |
          - name: http
            containerPort: 80
        envVars:
          APP_ENV: production
    service:
      enabled: true
      ports: |
        - name: http
          port: 80
          targetPort: 80

apps-ingresses:
  api:
    _include: ["apps-ingresses-defaultIngress"]
    host: "api.{{ $.Values.global.ci_url }}"
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

Связанные параметры:
- [`global.env`](reference-values.md#param-global-env)
- [`global._includes` + `_include`](reference-values.md#param-global-includes)
- [`containers`](reference-values.md#param-containers)
- [`service`](reference-values.md#param-service)
- [`ingress`](reference-values.md#param-ingress)

## 4. Проверить рендер локально

```bash
helm lint .helm
helm template my-app .helm --set global.env=prod > /tmp/rendered.yaml
```

Минимальная проверка результата:
1. В `/tmp/rendered.yaml` есть `Deployment`.
2. В `/tmp/rendered.yaml` есть `Service`.
3. В `/tmp/rendered.yaml` есть `Ingress`.
4. `api` использует ожидаемый `image` и `replicas`.

## 5. Частые следующие шаги

1. Добавить секреты как env: [`secretEnvVars`](reference-values.md#param-secretenvvars), пример в [Cookbook 6](cookbook.md#example-secretenvvars).
2. Подключить общий Secret/ConfigMap: [`sharedEnvSecrets`](reference-values.md#param-sharedenvsecrets), [`sharedEnvConfigMaps`](reference-values.md#param-sharedenvconfigmaps).
3. Добавить autoscaling: [`horizontalPodAutoscaler`](reference-values.md#param-hpa), пример в [Cookbook 10](cookbook.md#example-hpa).
4. Включить release matrix: [`global.deploy` + `global.releases`](reference-values.md#param-global-deploy).

## 6. Три базовых шаблона для старта

### 6.1 API сервис

Используйте рецепт: [Cookbook 1](cookbook.md#example-basic-api).

### 6.2 Worker без Service

Используйте рецепт: [Cookbook 3](cookbook.md#3-worker-без-service).

### 6.3 CronJob

Используйте рецепт: [Cookbook 4](cookbook.md#example-cronjob).

## 7. Если что-то не работает

1. Сначала откройте [operations.md](operations.md) и пройдите triage по слоям.
2. Проверьте типы параметров в [reference-values.md](reference-values.md).
3. Сверьтесь с типовым примером в [cookbook.md](cookbook.md).

Навигация: [Наверх](#top)
