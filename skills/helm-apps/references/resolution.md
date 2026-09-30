# Как из values получается значение

## `_include`: композиция на уровне нужного узла

`global._includes` — реестр YAML-профилей; `_include` подключает профиль в текущую map. Профиль workload содержит `containers`/`service`; профиль container — `image`/`resources`/`envVars`. Это позволяет менять общую политику, не повторяя её в каждой app.

```yaml
global:
  _includes:
    workload-base:
      enabled: true
      replicas:
        _default: 1
        production: 3
    small-container:
      resources:
        requests: {mcpu: 100, memoryMb: 128}
    burst:
      replicas:
        production: 5

apps-stateless:
  api:
    _include: [workload-base, burst]
    containers:
      main:
        _include: [small-container]
        image: {name: nginx, staticTag: "1.27"}
```

В dev `replicas=1`, в production `5`. Локальное `replicas: 2` у api победит оба профиля во всех env. Применение `_include: [workload-base]` под `containers.main` добавило бы workload-поля на неверном уровне.

Правила **библиотечного** merge в 1.10.1:

| Значение | Результат |
|---|---|
| Scalar или YAML block string | Побеждает более приоритетное значение целиком |
| Обычная map | Рекурсивное объединение по ключам |
| Native list | Более приоритетный список заменяет весь нижний; нижний наследуется только при отсутствующем ключе |
| Служебные `_include`-цепочки | Конкатенация с сохранением порядка |
| Env-map со scalar-ветками | Унаследуются отсутствующие env-ключи; конфликтующая ветка берётся сверху |

Особый случай: если нижняя map имеет `_default`, конфликтующие **map-valued env-ветки** берутся целиком сверху. Нижняя `production: {host: db, port: 5432}` и верхняя `production: {host: canary}` дают `{host: canary}`, без port. Задавай такую ветку полностью. Это правило отличается от обычной структурной map и от Helm merge.

Профили могут подключать другие профили. Держи цепочку короткой, называй профили по роли (`http`, `migration`, `observability`), избегай циклов. YAML anchors удобны внутри одного файла, но не заменяют библиотечные профили между файлами.

## Зачем нужен `fl.value`

`fl.value` — интерпретатор scalar-части DSL библиотеки. Он выбирает env-ветку, раскрывает короткие value refs, затем выполняет Go template, если строка содержит `{{`. Доступ к `.Values` возвращает исходную структуру и сам этот процесс не запускает.

```yaml
global:
  env: production
  vars:
    domain:
      _default: dev.example.test
      production: example.test
    port: 8080

# Фрагмент containers.main:
envVars:
  DOMAIN: '$fl.value{global.vars.domain}'
  PUBLIC_URL: 'https://{{ $.CurrentApp.name }}.{{ include "fl.value" (list $ . $.Values.global.vars.domain) }}'
ports: |
  - name: http
    containerPort: {{ include "fl.value" (list $ . $.Values.global.vars.port) }}
```

В production `DOMAIN=example.test`; порт в итоговом YAML — число `8080`. `{{ $.Values.global.vars.domain }}` напечатал бы map вместо выбранной строки. Внутри `PUBLIC_URL` явный helper разрешает другое значение, вложенное в составной шаблон.

Сигнатура: `include "fl.value" (list $ $scope $value)`. Результат всегда строка. Map аргумента считается env-map; выбранная map/list не сериализуется и даёт пустой результат. Для Kubernetes block разрешай строку, для структурного content — подходящий renderer/helper.

Env: exact key → один regex → `_default`. Regex сравнивается со всем именем env; явные `^stage-.*$` легче читать. Несколько regex-match дают ошибку, а отсутствие всех вариантов у `fl.value` даёт пустоту. Поэтому обязательное поле должно иметь fallback или явную проверку.

Короткие refs `$fl.value{global.vars.port}` начинаются от `.Values`, поддерживают вложенные ссылки и подстановку внутри текста. Сегменты пути — `[A-Za-z0-9_-]+`; для ключей с точками или индексирования используй Go template с `index`. `$$fl.value{...}` оставляет literal. Цикл, отсутствующий путь и неверная syntax дают диагностические ошибки. Действует только в полях, проходящих через `fl.value` или его wrappers.

## Когда шаблон улучшает читаемость

Используй литерал для самостоятельного факта, env-map для отличия стендов, короткую ref для общего значения. Шаблон полезен, когда выражает **зависимость**: имя ресурса от app, backend от service name, URL от env-domain, список портов от единственного общего порта. Так изменение источника автоматически обновляет связанные поля.

Оставляй выражение коротким. Повторяющийся алгоритм выноси в named helper в `templates/_helpers.tpl`, большой конфиг — в chart file. Большие `if/range/set` в values скрывают поведение; для повторяемого доменного правила лучше custom renderer с проверками.

## Root и local scope

| Контекст | Что использовать |
|---|---|
| Корень Helm | `$.Values`, `$.Release`, `$.Capabilities`, `$.Files` |
| App, после вычисления её имени | `$.CurrentApp` |
| Во время обработки контейнера | `$.CurrentContainer` |
| Только внутри child app | `$.ParentApp` |
| `.` в tpl, запущенном `fl.value` | Переданный relative scope: app, container, config-file node и т.д. |

В `envVars` локальная точка обычно container; в `configFiles.*.content` — file configuration. Явный `$.CurrentContainer.name` понятнее предположения о `.name`. Имя app вычисляется **до** установки `CurrentApp`: для `name` используй ключ `.__AppName__`, root values или Release. `name: '{{ .name }}'` ссылается на собственную невычисленную строку, а CurrentApp здесь может дать предыдущую app. Например, `name: '{{ printf "%s-%s" $.Release.Name .__AppName__ }}'` связывает имя с релизом. File import происходит ещё раньше, поэтому его пути опираются на root values, не app context.

## Тип поля важнее внешнего вида YAML

| Поле | Удобная форма |
|---|---|
| `replicas`, `enabled`, image tag, одна `envVars` | Scalar или env-map scalar |
| `ports`, probes, annotations, affinity, `args` | YAML block string; при отличии стендов env-map строк |
| `_include`, `_include_files`, sharedEnvSecrets/ConfigMaps | Native list по подтверждённому контракту |
| `envYAML` | Структурная map; `_default` отмечает env-лист переменной |
| `configFilesYAML.*.content` | Структурная map; `_default` отмечает env-лист, сохраняются native типы |
| Native `NetworkPolicy.spec` | Выбирается корневой env-map, вложенные поля сохраняются как raw data |
| Generic `apps-k8s-manifests` | Контракт конкретного поля; native list элементы — raw data без env/tpl внутри |

Scalar в `envVars` переопределяет одноимённую структурную env-переменную. Если оба источника env-map, их ветки merge по ключам: local `_default` не удаляет унаследованную exact production-ветку. Для переопределения production задай именно production либо scalar для всех стендов.

В `configFilesYAML` выбранный `null` удаляет ключ, пустые maps очищаются. Для tpl внутри его content используй лист `{_default: '{{ ... }}'}`: обычные строковые structural leaves не проходят tpl. Выбранные native lists атомарны; шаблоны в их элементах автоматически не вычисляются.

У generic `apps-k8s-manifests` свой recursive resolver: в 1.10.1 он проверяет ambiguity regex до выбора exact, поэтому два совпадающих regex могут дать ошибку даже при exact-ветке. Правило exact-first выше описывает `fl.value`; для generic fields также обеспечивай непересекающиеся regex и проверяй конкретный renderer.

Experimental native-list opt-in не делает `containerPort: '{{ ... }}'` числом. Block string даёт renderer сначала выполнить шаблон, затем разобрать YAML с правильными типами. Поведение конкретного поля проверяй рендером.

Источники 1.10.1: [value](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/fl-functions/_value.tpl), [merge](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/fl-functions/_expandIncludesInValues.tpl), [структурные helpers](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-helpers.tpl).
