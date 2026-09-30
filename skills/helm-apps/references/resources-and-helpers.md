# Используй подходящий renderer и расширяй по необходимости

## Возможности `apps-*`

Это карта выбора, а не замена схемы. У каждого renderer свои разрешённые поля и CRD prerequisites.

| Задача | Группа/возможность |
|---|---|
| Постоянные stateless pods | `apps-stateless` → Deployment |
| Stateful pods и persistent storage | `apps-stateful` → StatefulSet; проверь storage contract, имя `persistantVolumes` историческое |
| Pod на каждом узле | `apps-daemonsets` → DaemonSet; rollout через updateStrategy, без replicas/strategy |
| Разовая/периодическая задача | `apps-jobs` / `apps-cronjobs`; command/args/retries/schedule, SA и config файлы |
| Входящий трафик | `apps-services` или workload.service; `apps-ingresses`, TLS, Dex auth |
| Изоляция трафика | `apps-network-policies` с type kubernetes/cilium/calico и соответствующими полями |
| Конфиги/секреты/диски/лимиты | `apps-configmaps`, `apps-secrets`, `apps-pvcs`, `apps-limit-range` |
| Явный RBAC | `apps-service-accounts`; workload связывается с именем SA |
| Сертификаты и identity | `apps-certificates`, `apps-dex-clients`, `apps-dex-authenticators` |
| Мониторинг | `apps-custom-prometheus-rules`, `apps-grafana-dashboards` — проверь конкретный CRD, здесь Deckhouse kinds |
| Kafka/infra | `apps-kafka-strimzi` для Strimzi; `apps-infra` для поддержанных infra сущностей |
| Свой Kubernetes kind/CRD | `apps-k8s-manifests`; при повторяемой доменной логике — custom renderer |

Workloads могут создавать service, HPA/VPA/PDB, managed config/secret files и env. Сначала используй соответствующее поле библиотеки: оно учитывает контекст и API compatibility. Raw `extraSpec`, `podSpecExtra`, `extraFields`, `jobTemplateExtraSpec` полезны для неподдержанных полей, но обходят Kubernetes gates — проверяй их по целевой API schema.

Для env выбери минимально достаточный механизм:

- `envVars` — обычные scalar/env-map variables.
- `envYAML` — структурная конфигурация, превращаемая в env names; `_default` отмечает лист.
- `fromSecretsEnvVars` — ключи внешнего Secret; `sharedEnvSecrets`/`sharedEnvConfigMaps` — общие sources.
- `secretEnvVars` — Secret, управляемый библиотекой; создание и checksum участвуют в rollout.
- `configFiles`, `configFilesYAML`, `secretConfigFiles` — mount с generated либо существующим ресурсом.

Приоритет envFrom sources от низкого к высокому: shared ConfigMaps → shared Secrets → explicit envFrom → managed Secret env. Explicit `env` entries имеют Kubernetes приоритет над envFrom. Дубли конкретных переменных и их типы проверь в итоговом PodSpec.

## `childApps`: зависимые ресурсы возле workload

```yaml
apps-stateless:
  api:
    enabled: true
    # containers описаны здесь либо пришли из профиля
    childApps:
      apps-configmaps:
        runtime:
          enabled: true
          name: '{{ $.ParentApp.name }}-runtime'
          data: |
            owner: {{ $.ParentApp.name | quote }}
```

Child рендерится в контексте parent, только когда parent рендерится. Это удобно для app-specific ConfigMap/Ingress/Certificate. Поля parent не копируются автоматически; ссылку выражает `ParentApp`. Это расположение в values само по себе не задаёт Kubernetes ownerReferences или garbage collection.

Разрешённые child groups: certificates, configmaps, ingresses, k8s-manifests, network-policies, pvcs, secrets, service-accounts, services с префиксом `apps-`. Общий ресурс нескольких apps лучше объявить standalone, чтобы он не зависел от enabled одного parent.

## Custom group для читаемой организации

Группа может выражать домен (`payments`, `support`) и использовать готовые renderers:

```yaml
payments:
  __GroupVars__:
    type: apps-stateless
  api:
    _include: [workload-http]
  public:
    __AppType__: apps-ingresses
    enabled: true
    host: '$fl.value{global.vars.domain}'
    paths: |
      - path: /
        pathType: Prefix
        backend:
          service:
            name: api
            port:
              number: 8080
```

`__GroupVars__.type` может быть env-map. При смене renderer между стендами проверь обе API: одинаковые значения могут иметь разный контракт. `__AppType__` переопределяет renderer для одной app. Произвольные GroupVars не становятся app defaults: defaults задаются профилем, group variables доступны через `$.CurrentGroupVars`.

## Generic manifest или собственный renderer

Для одного CRD начни с `apps-k8s-manifests`:

```yaml
apps-k8s-manifests:
  bucket:
    enabled: true
    apiVersion: example.test/v1
    kind: Bucket
    spec: |
      region: {{ include "fl.value" (list $ . $.Values.global.vars.region) | quote }}
      policies:
        - name: {{ $.CurrentApp.name | quote }}
```

`example.test/v1` здесь условный CRD, не обещание установленного контроллера. Native map допустима по generic contract, но её native list elements остаются raw data: nested tpl удобнее писать block string. Формат неизвестного CRD проверяй его настоящей схемой.

Custom renderer оправдан, когда много apps описывают одну доменную сущность, а named helper может выразить её правила и ошибки лучше repeated raw YAML. Он определяется в consumer `templates/`, библиотеку форкать не нужно:

```yaml
runtime-settings:
  __GroupVars__:
    type: demo-configmaps
  endpoint:
    enabled: true
    endpoint:
      _default: https://dev.example.test
      production: https://example.test
    emitDetails:
      _default: false
      production: true
```

```gotemplate
{{- define "demo-configmaps.render" -}}
{{- $app := $.CurrentApp -}}
apiVersion: v1
kind: ConfigMap
{{ include "apps-helpers.metadataGenerator" (list $ $app) }}
data:
  endpoint: {{ include "fl.value" (list $ $app $app.endpoint) | quote }}
{{ if include "fl.isTrue" (list $ $app $app.emitDetails) }}
  details: "enabled"
{{ end }}
{{- end -}}
```

Init вызывает `<type>.render` с root контекстом, установленными CurrentApp/GroupVars/Group/Path и проверенным app.enabled. Собственные поля не resolve автоматически: renderer выбирает helpers. Собственную schema добавь в consumer для доменных полей — библиотечная схема не валидирует весь custom contract.

## Практические helpers

| Helper | Сигнатура | Для чего |
|---|---|---|
| `fl.value` | `(list $ $scope $value)` | Env/ref/tpl scalar |
| `fl.valueQuoted`, `fl.valueSingleQuoted` | те же args | Resolve, затем quote; пустой result не печатается |
| `fl.isTrue` | те же args | Безопасное условие для resolved true |
| `apps.value` | `(list $ $scope $value "field")` | Value с field context в диагностике |
| `apps-utils.requiredValue` | `(list $ $scope "field")` | Resolve+trim+ошибка при пустом обязательном поле |
| `apps-helpers.metadataGenerator` | `(list $ $app)` | Весь `metadata:`, labels/annotations библиотеки |
| `fl.generateSelectorLabels` | `(list $ $scope $appName)` | Библиотечный selector label; сравни с настоящим workload |
| `apps.generateConfigMapData` | `(list $ $scope $data)` | Quote scalar entries и resolve их с путём data |
| `fl.formatStringAsDNSLabel`, `fl.formatStringAsDNSSubdomain` | строка, не list | Нормализация имени и ограничение длины; не заменяет все проверки DNS |

Например, данные custom ConfigMap можно передать `include "apps.generateConfigMapData" (list $ $app $app.data) | nindent 2`. У `apps-utils.generateSpecs` есть typed emitter для Strings/Numbers/Maps/Lists/Bools/Required; Strings уже выводятся quoted. Читай define и вызывающие renderers установленной версии перед использованием.

`if include "fl.value" ...` с boolean false проверяет непустую строку `"false"` и входит в ветку. Пользуйся `fl.isTrue`; `fl.isFalse` проверяет «не равно true», включая пустоту, и не является строгим validator literal false. После `fl.value` при необходимости преобразуй строку в число/boolean **в выходном YAML**, не считай include typed result.

`fl.generateContainer*`, `apps.generate*`, pod/metadata helpers позволяют custom renderers переиспользовать библиотечную механику. Они требуют ожидаемых CurrentApp/CurrentContainer и scope; изучи реальный caller, а не только название. `_fl.*`/`_apps-*` и mutation walkers — implementation details. Даже найденный `define` может быть внутри комментария (`fl.percentage` в 1.10.1); каталог — индекс поиска, существование helper подтверждает реальный render.

Pre-render hooks способны менять контекст, включая enabled, до проверки app. Обычные env-map/profiles/templates легче проследить; hook используй для подтверждённой потребности и тестируй изменения состояния между apps.

## Release matrix: общая версия набора apps

Полезна, когда версия поставки содержит согласованный набор image tags. Нужны `global.deploy.enabled`, `release`, `global.releases`; `versionKey` отделяет lookup key от app name. Static tag контейнера сильнее matrix fallback:

```yaml
global:
  deploy:
    enabled: true
    release: release-2026-10
    autoEnableApps: true
    annotateAllWithRelease: true
  releases:
    release-2026-10:
      api: "1.28"

apps-stateless:
  api:
    enabled: false
    containers:
      main:
        image:
          staticTag: null
```

Overlay должен удалить staticTag также из подключённых профилей, если он оттуда наследуется. Проверенный [release overlay](../assets/consumer/values/release.yaml) показывает оба источника. `autoEnableApps` принудительно включает найденную app даже с false. App, отсутствующая в matrix, **не выключается автоматически**: для selective rollout ей нужны false defaults. Shared resources включай явно. Matrix fallback относится к обычным containers; initContainers имеют собственный image contract. Библиотека добавляет release/app-version annotations, managed generated resources получают соответствующие metadata.

Источники 1.10.1: [render/init](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-utils.tpl), [metadata/container helpers](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-helpers.tpl), [release](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-release.tpl), [generic renderer](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-k8s-manifests.tpl).
