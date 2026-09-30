# Читаемая структура чарта и стендов

Начни с одного values, если приложения и различия помещаются на экране. При росте разделяй **по смыслу**, а не каждое поле в отдельный файл:

```text
.helm/
  Chart.yaml
  values.yaml                 # env, общие vars, реестр профилей, карта групп
  profiles/base.yaml          # общая политика workload/container
  profiles/http.yaml          # HTTP ports/service, отдельная роль
  apps/api.yaml               # api/admin: подключённые роли и отличия apps
  files/nginx.conf.tpl        # большой конфиг в собственном языке
  templates/apps.yaml         # ровно один init-library
  templates/_custom.tpl       # только если нужен собственный renderer
deploy/
  production.yaml             # Helm overlay конкретной площадки
  canary.yaml                 # небольшой overlay поверх неё
```

`global.vars` удобно использовать для собственных project-level переменных: domain, port, registry. Отделяй их от `global._includes`, где лежат reusable YAML-фрагменты. Дай человеку видимую цепочку: **app → короткий профиль → глобальная переменная/ветка env**. Не переносить всё в профили: уникальные настройки app читабельнее рядом с app.

## Способы чтения файлов

| Механизм | Что лежит в файле | Когда применяется |
|---|---|---|
| Helm `-f path.yaml` | Полный values overlay с `global`/`apps-*` | До вызова библиотеки |
| `_include_from_file: path.yaml` | YAML map **для текущего узла** | File import библиотеки; local узел сильнее |
| `_include_files: [a.yaml, b.yaml]` | Профили **для текущего узла** | Превращаются в `_include`: b сильнее a, именованные `_include` идут после файлов |
| `.Files.Get` | Конфиг, сертификат, SQL или иной текст | Во время tpl нужного поля; само чтение не выполняет tpl внутри файла |

```yaml
# .helm/values.yaml
global:
  env: dev
  _includes:
    workload:
      _include_from_file: profiles/base.yaml
    observability:
      annotations: |
        example.test/team: platform

apps-stateless:
  _include_from_file: apps/api.yaml
```

```yaml
# .helm/apps/api.yaml: без внешнего apps-stateless wrapper
api:
  _include_files: [profiles/http.yaml]
  _include: [workload, observability]
  replicas: 2
```

Map профилей может быть загружена и целиком: `global._includes._include_from_file: profiles/all.yaml`; тогда файл содержит именованные профили. Выбирай один понятный способ регистрации.

Несколько app-файлов одной группы можно соединить без монолитного списка:

```yaml
apps-stateless:
  _include_files: [apps/api.yaml, apps/workers.yaml]
```

Каждый файл содержит карту app entries; если имена совпадают, поздний файл имеет больший приоритет. Для independent domain group можно дать собственное имя и `__GroupVars__.type`, как в [resources-and-helpers.md](resources-and-helpers.md).

Пути `_include_*` и `.Files.Get` — внутри **consumer chart**, включая файлы из него в package. `-f ../deploy/production.yaml` читает файловую систему CLI и не меняет базу библиотечных путей. `templates/`, файлы вне доступного chart и исключённые `.helmignore` недоступны `.Files`. Имя файла может быть tpl от root values, например `profiles/{{ $.Values.global.env }}.yaml`; для небольших отличий проще один профиль с env-map. [Ограничения Helm Files](https://helm.sh/docs/chart_template_guide/accessing_files/).

В 1.10.1 missing/empty `_include_*` file пропускается. Для обязательных файлов добавь до init понятную проверку:

```gotemplate
{{- range $path := list "profiles/base.yaml" "apps/api.yaml" -}}
  {{- if not ($.Files.Get $path | trim) -}}
    {{- fail (printf "Required chart file missing or empty: %s" $path) -}}
  {{- end -}}
{{- end -}}
{{- include "apps-utils.init-library" $ }}
```

Внешний файл с Go template должен сначала быть валидной YAML map: строку `{{ ... }}` заключай в кавычки или block string. Импорт не является `tpl` всего YAML-файла; шаблонные поля разрешаются позже их renderer.

## Выбор: env-map или Helm overlay

Env-map возле поля удобен для небольшой стабильной матрицы:

```yaml
replicas:
  _default: 1
  production: 3
  "^stage-.*$": 2
```

Overlay удобен для независимого кластера/заказчика, большого набора отличий, данных CI или временного canary. Он содержит только изменённые поля и полный путь:

```yaml
# deploy/production.yaml
apps-stateless:
  api:
    replicas: 6
    containers:
      main:
        image:
          staticTag: "1.28"
```

```sh
helm template demo .helm \
  -f deploy/production.yaml -f deploy/canary.yaml \
  --set global.env=production --namespace demo --kube-version 1.29.0
```

Default values → overlays слева направо → CLI overrides. Maps Helm объединяет рекурсивно; lists и scalar/block strings заменяются целиком. Helm overlay с `_include` заменяет весь предыдущий список: конкатенация библиотеки относится к её profile merge. CLI `--set-string` подходит строковому image tag; `--set-file` — большому строковому полю. Они не запускают библиотечную обработку сами по себе. [Команда Helm template](https://helm.sh/docs/helm/helm_template/).

После Helm merge библиотека ещё читает свои файлы. Поэтому overlay **локального app-поля** побеждает значение, импортированное из apps-файла. Overlay профиля может не повлиять на app, если поле уже переопределено локально: смотри итоговую цепочку источников.

Не смешивай scalar и env-map случайно: замена `replicas: {_default:1, production:3}` на scalar `6` задаст шесть для всех env этого набора values. Изменение только `replicas.production: 6` сохраняет остальные env, **если исходное поле уже env-map**.

Если профиль содержит строковую ссылку `replicas: '$fl.value{global.vars.replicas}'`, новый map с единственной production-веткой заменит ссылку целиком. Dev останется без значения replicas, хотя lint/render могут пройти. Для override только production сохрани fallback явно:

```yaml
apps-stateless:
  api:
    replicas:
      _default: '$fl.value{global.vars.replicas}'
      production: 6
    containers:
      main:
        image:
          staticTag:
            _default: "1.27"
            production: "1.28"
```

Этот overlay безопасно передавать с любым env. Dev берёт исходную ref, production — override. В настоящем проекте исходный image default тоже можно вынести в global vars и сослаться на него. Проверь dev и production с overlay и без него. При переходе map↔scalar Helm не всегда предупреждает; удаление источника через `null` может позволить библиотечному профилю снова заполнить отсутствующий ключ.

## Конфигурационные файлы

```yaml
# containers.main
configFiles:
  nginx.conf:
    mountPath: /etc/nginx/nginx.conf
    content: |
      {{- tpl (required "files/nginx.conf.tpl is required" ($.Files.Get "files/nginx.conf.tpl")) $ }}
```

Файл `files/nginx.conf.tpl` хранит читаемый nginx-конфиг. Содержимое может ссылаться на CurrentApp/CurrentContainer и вызывать fl.value. У обычного `.Files.Get` вложенные `{{ ... }}` остаются текстом; явный `tpl` вычисляет их. Библиотека создаёт ConfigMap, volume и mount. Для имеющегося ConfigMap/Secret задай `name` вместо `content`; имя тоже может быть env-map/tpl. Volume names должны быть уникальны среди workload/container/init/managed источников.

`configFilesYAML` лучше подходит структурному YAML с env-листьями и типами:

```yaml
configFilesYAML:
  application.yaml:
    mountPath: /etc/app/application.yaml
    content:
      server:
        timeoutSeconds: {_default: 30}
        hostname: {_default: '{{ $.CurrentApp.name }}'}
      debug:
        _default: true
        production: null
      peers:
        _default: [localhost]
        production: [db-a, db-b]
```

Prod убирает debug, timeoutSeconds остаётся числом, peers заменяется целиком. Для JSON из **статического** YAML можно `Files.Get | fromYaml | toPrettyJson`; это сериализация, она не выбирает env-map. Для env-aware JSON сначала разреши структуру подтверждённым helper с подходящим контекстом и проверь результат, либо держи читаемый JSON template в файле. Если вычисленное значение внутри YAML должно остаться числом, текстовый `configFiles` template часто удобнее: `fl.value` в строковой ветке structural content даёт строку.

Не помещай настоящие секреты в публикуемый chart. Для существующих секретов используй external references; managed secret content передавай способом, выбранным проектом. Rendered YAML тоже может содержать секретные данные: сравнивай нужные поля, сохраняя привычные правила проекта.

## Цельный пример

[assets/consumer](../assets/consumer/values.yaml) показывает два HTTP app, файл профиля, именованные профили, app-file import, external tpl-конфиг, child ConfigMap и structural YAML. Дополнительные overlays демонстрируют production/canary, release matrix и custom renderer. Это учебный chart: его имена, nginx, endpoints и версия зависимости служат проверяемому примеру, а не готовому production стандарту.

Скопируй consumer в нужную chart directory, адаптируй поля и pin версии, затем собери зависимость обычным workflow проекта. Проверь пример локальным checkout:

```sh
ruby scripts/verify_examples.rb /path/to/helm-apps/charts/helm-apps
```

Источники 1.10.1: [file imports/init](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-utils.tpl), [config-file generation](https://github.com/alvnukov/helm-apps/blob/helm-apps-1.10.1/charts/helm-apps/templates/_apps-components.tpl).
