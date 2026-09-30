---
name: helm-apps
description: Use when creating, refactoring or debugging consumer charts built on helm-apps, especially _include profiles, env-specific values, file imports, Helm overrides, apps-* resources or custom renderers.
---

# Helm Apps

Собирай чарт, в котором человек может проследить каждое важное значение до его источника. Используй возможности библиотеки для устранения повторений и связанных ошибок; добавляй абстракцию, когда она объясняет конфигурацию.

## Сначала установи контракт

1. Найди consumer `Chart.yaml`, `Chart.lock`, установленную зависимость и единственный вызов `apps-utils.init-library`. Установленная версия важнее диапазона в `Chart.yaml`. Эти примеры проверены на **1.10.1**; особенности другой версии сверяй с её исходниками.
2. Если доступен happ, начни с `helm_apps(op="overview")`. Для конкретной app используй `resolve`, `origin`, `diff`, затем `render`. Сравни embedded/declared/installed версии; при расхождении окончательное доказательство — реальный Helm с зависимостью consumer.
3. Читай только нужную часть схемы и соответствующий renderer. При доступном checkout источники: `tests/.helm/values.schema.json`, `charts/helm-apps/templates/`, `tests/contracts/`, `docs/reference-values.md`, `docs/ai/helm-apps-capabilities.prompt.md`. В release archive документации может не быть. При противоречии источников покажи его и подтверди гипотезу минимальным рендером.

## Выбери способ выражения

| Потребность | Открыть |
|---|---|
| Повторения, `_include`, env-map, `fl.value`, шаблоны и типы | [Разрешение значений](references/resolution.md) |
| Разделение файлов, стенды, Helm `-f`, внешние конфиги | [Структура и файлы](references/layout-and-files.md) |
| Выбор `apps-*`, дочерние ресурсы, release matrix, собственные renderers/helpers | [Ресурсы и расширения](references/resources-and-helpers.md) |
| Диагностика, источники контракта и проверка результата | [Проверка](references/verification.md) |

Порядок обработки: **Helm values/overrides → файловые импорты библиотеки → профили `_include` → app context и env/tpl нужных полей → Kubernetes YAML**. Env-разрешение происходит при чтении renderer/helper, а не глобально для каждого YAML-узла.

## Рабочие правила

- Одна настройка — один смысловой источник. Небольшие различия стендов оставляй env-map возле поля; общие роли приложений выноси в короткие профили; независимые конфигурации площадок оформляй Helm overlays.
- Профиль соответствует узлу подключения: workload, container, service. Более поздний `_include` сильнее предыдущего; local values сильнее профилей. Helm `-f` имеет отдельную семантику.
- Kubernetes maps/lists обычно задавай `|`; `_include`/`_include_files` — native lists. Структурные исключения проверяй по полю. Шаблон в block string позволяет получить число/boolean после рендера.
- Для scalar/env-map используй `fl.value`; для простого обращения — `$fl.value{global.vars.port}`. Обычный доступ к `.Values` не выбирает env-ветку. `fl.value` возвращает строку, не сериализует map/list; boolean-проверку делай `fl.isTrue`.
- `global.env` выбирает стенд в Helm; в werf согласуй с `--env`, поскольку `werf.env` имеет приоритет. Exact → единственный regex → `_default`. В структурных `envYAML`/`configFilesYAML` `_default` также отмечает env-лист.
- Пути библиотечных файлов идут от корня consumer chart. `.Files.Get` читает содержимое; для шаблонов внутри файла нужен явный `tpl`. Используй `$` для корня; проверяй доступность `CurrentApp`, `CurrentContainer`, `ParentApp` в конкретной фазе.
- Сохраняй выбранные пользователем workflow и флаги совместимости. Строгую валидацию применяй как явную проверку; включение её в defaults — отдельное изменение поведения.

## Внеси и докажи изменение

Перед правкой сформулируй ожидаемый результат для изменённых стендов и ресурсных полей. После правки выполни lint и реальный render с теми же `-f`, release name, namespace и Kubernetes version, которые использует проект. Проверь итоговые имена, selectors, типы, ссылки, mounts и объём ресурсов; при рефакторинге сравни manifests до/после. Команды и пределы проверки — в [verification.md](references/verification.md).

Для нового consumer адаптируй [проверенный пример](assets/consumer/values.yaml); разбор структуры — в [layout-and-files.md](references/layout-and-files.md). Проверка примера без сети и кластера:

```sh
ruby scripts/verify_examples.rb /path/to/helm-apps/charts/helm-apps
```

Пути здесь относительно каталога скилла. Скрипт работает в temporary chart. Завершая задачу, покажи человеку изменённые файлы, существенную цепочку источников значений и реально выполненные проверки.
