# Helm Apps Library

Библиотека Helm-шаблонов для стандартизированного деплоя приложений в Kubernetes.

`helm-apps` позволяет описывать приложения через `values.yaml` без копирования шаблонов между сервисами.
Логика рендера централизована в библиотеке, а сервисные репозитории хранят только конфигурацию.

> Библиотека полностью поддерживает Helm и совместима с werf.  
> Практически, для командного daily workflow werf часто удобнее: он объединяет рендер и процесс поставки в единый поток, снижая количество ручных шагов в CI/CD.  
> При этом весь функционал библиотеки доступен и через чистый Helm.

История изменений: [`CHANGELOG.md`](CHANGELOG.md)

## Зачем использовать библиотеку

- Единый стандарт деплоя для всех сервисов команды.
- Меньше копипаста и ручных Kubernetes-манифестов.
- Быстрее ревью: одинаковая структура конфигов между проектами.
- Переиспользование через [`_include`](docs/parameter-index.md#core) и [`global._includes`](docs/parameter-index.md#core).
- Поддержка окружений через [`global.env`](docs/parameter-index.md#core) (`_default`, env overrides, regex env keys).
- Режим релиз-матрицы через [`global.release`](docs/reference-values.md#param-global-release) для автоподстановки тегов и централизованного переключения версий.
- Поддержка связанных ресурсов (Service, Ingress, ConfigMap, Secret, HPA, VPA, PDB и др.) в одной модели.

## Какие ресурсы поддерживаются

- `apps-stateless` (`Deployment`)
- `apps-stateful` (`StatefulSet`)
- `apps-jobs` (`Job`)
- `apps-cronjobs` (`CronJob`)
- `apps-services` (`Service`)
- `apps-ingresses` (`Ingress`)
- `apps-network-policies` (`NetworkPolicy`)
- `apps-configmaps` (`ConfigMap`)
- `apps-secrets` (`Secret`)
- `apps-pvcs` (`PersistentVolumeClaim`)
- `apps-limit-range` (`LimitRange`)
- `apps-certificates` (`Certificate`)
- `apps-dex-clients`, `apps-dex-authenticators`
- `apps-custom-prometheus-rules`, `apps-grafana-dashboards`
- `apps-kafka-strimzi`
- `apps-infra`

Для `apps-network-policies` можно выбрать API через `type`:
- `kubernetes` (default) -> `networking.k8s.io/v1`, `NetworkPolicy`
- `cilium` -> `cilium.io/v2`, `CiliumNetworkPolicy`
- `calico` -> `projectcalico.org/v3`, `NetworkPolicy`
- для любого другого CNI можно явно задать `apiVersion`, `kind` и `spec`.

## Быстрый старт

### 1. Подключить dependency

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

### 2. Добавить инициализацию библиотеки

Создать `.helm/templates/init-helm-apps-library.yaml`:

```yaml
{{- include "apps-utils.init-library" $ }}
```

### 3. Обновить зависимости

```bash
helm repo add --force-update helm-apps https://alvnukov.github.io/helm-apps
helm dependency update .helm
```

### 4. Описать приложение в values

Минимальный пример:

```yaml
global:
  env: prod
  ci_url: example.org

apps-stateless:
  api:
    _include: ["apps-stateless-defaultApp"]
    containers:
      main:
        image:
          name: nginx
        ports: |
          - name: http
            containerPort: 80
    service:
      enabled: true
      ports: |
        - name: http
          port: 80

apps-ingresses:
  api:
    _include: ["apps-ingresses-defaultIngress"]
    host: "{{ $.Values.global.ci_url }}"
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

<a id="example-global-includes-merge"></a>
## Ключевая механика: `global._includes` и рекурсивный merge

`global._includes` — это библиотека переиспользуемых конфигурационных блоков.
Приложение подключает их через `_include`, после чего библиотека делает рекурсивный merge.

Базовый пример:

```yaml
global:
  _includes:
    profile-base:
      replicas: 2
      service:
        enabled: true
        ports: |
          - name: http
            port: 80
      containers:
        main:
          resources:
            requests:
              mcpu: 100
              memoryMb: 128
    profile-prod:
      replicas: 4
      containers:
        main:
          resources:
            limits:
              memoryMb: 512

apps-stateless:
  api:
    _include: ["profile-base", "profile-prod"]
    containers:
      main:
        image:
          name: nginx
```

Что важно:

1. Merge рекурсивный: вложенные map-структуры не заменяются целиком, а объединяются по ключам.
2. Порядок `_include` важен: каждый следующий профиль может переопределять предыдущий.
3. Локальные поля приложения имеют приоритет над значениями из include-блоков.
4. Это главный механизм DRY в библиотеке: стандартные профили задаются один раз и переиспользуются во всех сервисах.
5. Native YAML list в values запрещены (кроме `_include` и `_include_files`): для Kubernetes list-полей используйте YAML block string (`|`).

## Release mode (`global.release`)

Опциональный режим для централизованного управления версиями приложений:
- `global.release.enabled` по умолчанию `false`;
- задаете текущий релиз в `global.release.current`;
- храните матрицу `release -> app -> version` в `global.release.versions`;
- ключ приложения берется из `releaseKey`, а если он не задан — из имени приложения (`app.name`);
- `autoEnableApps` по умолчанию `true`;
- app получает `CurrentAppVersion`, и если `image.staticTag` не задан, тег берется из релизной матрицы;
- в рендер добавляются аннотации `helm-apps/release` и `helm-apps/app-version`.

Важно:
- если для app не найдена версия в `global.release.versions.<current>`, приложение рендерится по обычной логике;
- если не задан ни `image.staticTag`, ни `CurrentAppVersion`, используется стандартный путь через `Values.werf.image`.

Практический референс и пример: [`docs/reference-values.md#param-global-release`](docs/reference-values.md#param-global-release)

### Примеры merge-поведения

#### Пример 1: Рекурсивный merge map

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

Итог для `api.service`:
- `enabled: true`
- `headless: false`
- `ports: ...`

#### Пример 2: Порядок include (последний имеет приоритет)

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

Итог: `replicas: 5`.

#### Пример 3: Локальный override сильнее include

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

Итог: `replicas: 3`.

#### Пример 4: Env-map merge с `_default` и конкретным env

Пример:

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

Итоговое поведение:
- для `production` будет использовано значение `4` (из `base.production`);
- для env без явного ключа будет использовано `_default: 1` (из `canary._default`).

Практика:
- окружение передавайте через `global.env`;
- всегда проверяйте итоговый рендер в целевом env (`helm template ... --set global.env=<env>`);
- для критичных env-map лучше держать все нужные env-ключи явно в финальном профиле.

<a id="example-include-concat"></a>
#### Пример 5: `_include`-списки конкатенируются

Если include-профиль сам содержит `_include`, итоговый список объединяется.

```yaml
global:
  _includes:
    profile-a:
      _include: ["base-a"]
      replicas: 2
    profile-b:
      _include: ["base-b"]
      service:
        enabled: true

apps-stateless:
  api:
    _include: ["profile-a", "profile-b"]
```

Итоговый include-chain для `api` объединяет оба списка (`base-a` + `base-b`) и затем применяет локальные поля.

#### Пример 6: Что со списками

Важный нюанс библиотеки:
- специальные списки `_include` конкатенируются;
- обычные “списковые” параметры в большинстве случаев задаются как YAML-строки (`|`), а не как native list.

Поэтому merge для обычных списков как list-поведение обычно не используется.
Практика:
- задавайте списковые Kubernetes-блоки строкой YAML;
- итог проверяйте через `helm template`.

### 5. Проверить рендер

```bash
helm lint .helm
helm template my-app .helm --set global.env=prod
```

### 6. Совместимость с версиями Kubernetes

Библиотека автоматически учитывает версию Kubernetes через `.Capabilities`:
- выбирает подходящий `apiVersion` для `CronJob`, `PodDisruptionBudget`, `HorizontalPodAutoscaler`, `VerticalPodAutoscaler`;
- учитывает различия в полях `spec` между версиями (например, в `Service` и `StatefulSet`).
- поддерживает passthrough для редких/новых полей через:
  - `extraSpec` (ресурсный `spec`);
  - `podSpecExtra` (Pod template `spec`);
  - `jobTemplateExtraSpec` (`Job.spec` / `CronJob.spec.jobTemplate.spec`);
  - `extraFields` (top-level поля ресурса/контейнера).

Практика для проверки:
- новый кластер: `helm template ... --kube-version 1.29.0`
- legacy-кластер: `helm template ... --kube-version 1.20.15`

Текущий CI также проверяет рендер на нескольких версиях Kubernetes.

## Маршрут по документации

Стартовая точка:
- [docs/README.md](docs/README.md)

Подробные документы:
- Концепция и архитектура: [docs/library-guide.md](docs/library-guide.md)
- Полный справочник полей: [docs/reference-values.md](docs/reference-values.md)
- Быстрый индекс параметров (описание + примеры): [docs/parameter-index.md](docs/parameter-index.md)
- Use-case карта (задача -> параметр -> пример -> проверка): [docs/use-case-map.md](docs/use-case-map.md)
- Готовые шаблоны для типовых сценариев: [docs/cookbook.md](docs/cookbook.md)
- Эксплуатация, triage, rollback: [docs/operations.md](docs/operations.md)
- Краткие правила helper-паттернов: [docs/usage.md](docs/usage.md)

Практические артефакты:
- Полный рабочий пример values: [tests/.helm/values.yaml](tests/.helm/values.yaml)
- JSON Schema валидации values: [tests/.helm/values.schema.json](tests/.helm/values.schema.json)
- Готовый пример проекта: [docs/example](docs/example)

Быстрые ссылки на параметры:
- Индекс параметров: [docs/parameter-index.md](docs/parameter-index.md)
- `global.env`: [описание + пример](docs/parameter-index.md#core)
- `_include` / `global._includes`: [описание + примеры merge](docs/parameter-index.md#core)
- `containers` / `envVars` / `secretEnvVars`: [описание + примеры](docs/parameter-index.md#containers-envconfig)

## Для контрибьюторов библиотеки

При изменении возможностей библиотеки обновляйте синхронно:

1. шаблоны в `charts/helm-apps/templates`;
2. примеры в `tests/.helm/values.yaml`;
3. схему в `tests/.helm/values.schema.json`;
4. документацию в `docs/reference-values.md` и `docs/cookbook.md`.
