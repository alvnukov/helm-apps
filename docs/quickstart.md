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

### 2.1 Опционально: post-install подсказки через `NOTES.txt`

Helm печатает `templates/NOTES.txt` после `helm install`/`helm upgrade`.
Это удобное место для команд проверки релиза.

Добавьте в ваш чарт `.helm/templates/NOTES.txt`:

```txt
Release: {{ .Release.Name }}
Namespace: {{ .Release.Namespace }}

helm status {{ .Release.Name }} -n {{ .Release.Namespace }}
kubectl get pods -n {{ .Release.Namespace }}
kubectl get svc,ingress -n {{ .Release.Namespace }}
```

Рабочий пример в репозитории: [`tests/.helm/templates/NOTES.txt`](../tests/.helm/templates/NOTES.txt)

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

Где менять “переменные” в quickstart:

| Что хотите настроить | Путь в `values.yaml` | Пример значения | Справка |
|---|---|---|---|
| Окружение рендера | `global.env` | `prod` | [`global.env`](reference-values.md#param-global-env) |
| Базовый домен/URL | `global.ci_url` | `example.org` | [`global`](reference-values.md#param-global) |
| Имя приложения | `apps-stateless.<appName>`, `apps-ingresses.<appName>` | `api` | [`apps-* sections`](reference-values.md#param-apps-sections) |
| Реплики | `apps-stateless.<appName>.replicas` | `_default: 2`, `prod: 3` | [`containers / app contract`](reference-values.md#param-containers) |
| Образ контейнера | `apps-stateless.<appName>.containers.<container>.image` | `name: nginx`, `staticTag: "1.27.0"` | [`containers`](reference-values.md#param-containers) |
| Порты контейнера | `apps-stateless.<appName>.containers.<container>.ports` | `containerPort: 80` | [`containers`](reference-values.md#param-containers) |
| Переменные окружения контейнера | `apps-stateless.<appName>.containers.<container>.envVars` | `APP_ENV: production` | [`envVars`](reference-values.md#param-envvars) |
| Service | `apps-stateless.<appName>.service.*` | `enabled: true`, `ports: ...` | [`service`](reference-values.md#param-service) |
| Ingress | `apps-ingresses.<appName>.*` | `host`, `paths`, `tls.enabled` | [`ingress`](reference-values.md#param-ingress) |

Для worker/cron-сценариев `Service` и `Ingress` обычно не нужны.

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

## 5. Подключить config-файл в контейнер (ConfigMap mount)

Минимальный пример для контейнера:

```yaml
apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: nginx
          staticTag: "1.27.0"
        configFiles:
          nginx.conf:
            mountPath: /etc/nginx/nginx.conf
            content: |
              events {}
              http {
                server {
                  listen 80;
                  location / {
                    return 200 "ok";
                  }
                }
              }
```

Что делает библиотека:
1. Создает `ConfigMap` с файлом `nginx.conf`.
2. Добавляет `volume` + `volumeMount` в контейнер `main`.
3. Монтирует файл по `mountPath`.

Связанные параметры:
- [`configFiles`](reference-values.md#param-configfiles)
- [`configFilesYAML`](reference-values.md#param-configfilesyaml)
- [`secretConfigFiles`](reference-values.md#param-secretconfigfiles)
- Практические примеры: [Cookbook 8](cookbook.md#example-configfiles), [Cookbook 9](cookbook.md#example-configfilesyaml)

## 6. Частые следующие шаги

1. Добавить секреты как env: [`secretEnvVars`](reference-values.md#param-secretenvvars), пример в [Cookbook 6](cookbook.md#example-secretenvvars).
2. Подключить общий Secret/ConfigMap: [`sharedEnvSecrets`](reference-values.md#param-sharedenvsecrets), [`sharedEnvConfigMaps`](reference-values.md#param-sharedenvconfigmaps).
3. Добавить autoscaling: [`horizontalPodAutoscaler`](reference-values.md#param-hpa), пример в [Cookbook 10](cookbook.md#example-hpa).
4. Включить release matrix: [`global.deploy` + `global.releases`](reference-values.md#param-global-deploy).

## 7. Три базовых шаблона для старта

### 7.1 API сервис

Используйте рецепт: [Cookbook 1](cookbook.md#example-basic-api).

### 7.2 Worker без Service

Используйте рецепт: [Cookbook 3](cookbook.md#3-worker-без-service).

### 7.3 CronJob

Используйте рецепт: [Cookbook 4](cookbook.md#example-cronjob).

## 8. Если что-то не работает

1. Сначала откройте [operations.md](operations.md) и пройдите triage по слоям.
2. Проверьте типы параметров в [reference-values.md](reference-values.md).
3. Сверьтесь с типовым примером в [cookbook.md](cookbook.md).

Навигация: [Наверх](#top)
