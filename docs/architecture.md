# Helm Apps Library: Architecture
<a id="top"></a>

Документ описывает архитектуру рендера и контракт приоритетов.
Цель: объяснить, почему библиотека дает стабильный и предсказуемый результат.

Быстрая навигация:
- [Старт docs](README.md)
- [Handbook](library-guide.md)
- [Reference](reference-values.md)
- [Operations](operations.md)

## 1. Концепция

`helm-apps` разделяет:
- логику рендера (в библиотеке);
- конфигурацию приложений (в `values.yaml` сервиса).

Пользователь работает только с декларативными данными.
Библиотека гарантирует единообразный рендер через общий pipeline.

## 2. Pipeline рендера

1. Инициализация контекста (`apps-utils.init-library`).
2. Обход групп (`apps-*` + custom groups).
3. Разрешение типа группы (`__GroupVars__.type`, `__AppType__`).
4. Сбор app-конфига:
- применение `global._includes` через `_include`;
- рекурсивный merge map-структур;
- локальные overrides приложения.
5. Разрешение env-map значений через `global.env`.
6. Рендер встроенного или custom renderer (`<type>.render`).
7. Построение итоговых Kubernetes-манифестов.

## 3. Приоритеты merge

### 3.1 Include и локальные значения
<a id="arch-include-priority"></a>

Приоритеты:
1. ранние include-профили;
2. поздние include-профили;
3. локальные значения приложения.

Следствие:
- последний include переопределяет предыдущие;
- локальный app override переопределяет include.

Ссылка на пример: [README merge section](../README.md#example-global-includes-merge)

### 3.2 Env-map (`_default`, exact env, regex)
<a id="arch-env-resolution"></a>

Приоритеты выбора env-значения:
1. точное совпадение `global.env`;
2. regex-ключ;
3. `_default`.

Если regex-совпадений несколько, библиотека падает с ошибкой (ambiguous env).

Ссылка: [reference-values.md#param-global-env](reference-values.md#param-global-env)

### 3.3 Формирование env для контейнера
<a id="arch-container-env-order"></a>

Порядок формирования:
1. слой `envFrom`:
- `sharedEnvConfigMaps`
- `sharedEnvSecrets`
- `envFrom`
- auto-secret из `secretEnvVars`
2. слой явных `env`-переменных:
- `envYAML`
- `envVars`
- `env`
- `fromSecretsEnvVars`

Для одинакового имени переменной явные `env`-переменные имеют приоритет над источниками из `envFrom` (семантика Kubernetes).

Ссылка: [cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars](cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars)

## 4. Типы данных и контракт values

Важный контракт библиотеки:
- native YAML lists в values запрещены почти везде;
- list/map-блоки Kubernetes обычно передаются как YAML block string (`|`);
- исключения для native list: `_include`, `_include_files`, `sharedEnvSecrets`, `sharedEnvConfigMaps`.

Проверка обеспечивается schema и runtime-валидацией.

Ссылки:
- [reference-values.md#param-cheat-sheet](reference-values.md#param-cheat-sheet)
- [stability.md](stability.md)

## 5. Расширяемость

### 5.1 Custom groups и custom renderer
<a id="arch-custom-renderer"></a>

Расширение выполняется без форка библиотеки:
- создается собственная top-level group;
- задается `__GroupVars__.type`;
- добавляется шаблон `define "<custom-type>.render"`.

Контекст для custom renderer включает `$.CurrentApp`, `$.CurrentGroupVars`, `$.CurrentPath`, `$.Values`, `$.Capabilities`, `$.Release`.

Ссылка: [library-guide.md#param-custom-renderer](library-guide.md#param-custom-renderer)

### 5.2 API-совместимость Kubernetes

Библиотека опирается на `.Capabilities` и выбирает корректные API/поля под версию кластера.
Проверяется CI matrix и server-side dry-run.

Ссылка: [stability.md#param-k8s-api-compat](stability.md#param-k8s-api-compat)

## 6. Почему архитектура устойчивая

- единый rendering engine для всех сервисов;
- единый контракт values;
- автоматизированная проверка схемы и контрактов;
- проверка совместимости на разных версиях Kubernetes;
- контролируемая эволюция в ветке `1.x`.

Навигация: [Наверх](#top)
