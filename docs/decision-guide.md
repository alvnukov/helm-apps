# Helm Apps Decision Guide
<a id="top"></a>

Документ помогает быстро выбрать правильный путь конфигурации без лишних экспериментов.

Быстрая навигация:
- [Старт docs](README.md)
- [Quick Start](quickstart.md)
- [Use-Case Map](use-case-map.md)
- [Reference](reference-values.md)

## 1. Какой тип workload выбрать

| Задача | Рекомендуемая секция | Почему | Пример |
|---|---|---|---|
| HTTP/API, фоновые consumers с постоянными pod | `apps-stateless` | Простая масштабируемая модель на Deployment | [Cookbook 1](cookbook.md#example-basic-api) |
| Stateful сервис с диском (DB, queue, broker) | `apps-stateful` | StatefulSet + устойчивые тома и порядок запуска | [Cookbook 12](cookbook.md#example-stateful-pvc) |
| Одноразовая задача (миграция, init) | `apps-jobs` | Запуск до завершения с контролем retry | [Cookbook 5](cookbook.md#5-one-shot-job-migration) |
| Периодическая задача | `apps-cronjobs` | Cron schedule + история выполнений | [Cookbook 4](cookbook.md#example-cronjob) |

## 2. Как задавать значения по окружениям

| Сценарий | Подход | Параметры |
|---|---|---|
| Одно значение во всех env | scalar | [global.env](reference-values.md#param-global-env) |
| Разные значения для env | env-map с `_default` + явные env | [Env pattern](reference-values.md#param-global-env) |
| Много переиспользования | профили в `global._includes` + `_include` | [global._includes](reference-values.md#param-global-includes) |
| Централизованный rollout версий | release matrix | [global.deploy](reference-values.md#param-global-deploy) |

Базовое правило:
1. Начинайте со scalar.
2. Когда появляется второе окружение, переходите на env-map с `_default`.
3. Когда появляется дублирование между сервисами, переносите общее в `global._includes`.

## 3. Какой способ env-переменных выбрать

| Нужный результат | Что использовать | Пример |
|---|---|---|
| Обычные не-секретные переменные | `envVars` | [Cookbook 1](cookbook.md#example-basic-api) |
| Секреты, создаваемые библиотекой | `secretEnvVars` | [Cookbook 6](cookbook.md#example-secretenvvars) |
| Секрет из внешнего Secret | `fromSecretsEnvVars` | [Cookbook 7](cookbook.md#example-fromsecretsenvvars) |
| Подключить общий Secret в несколько контейнеров | `sharedEnvSecrets` + `apps-secrets` | [Cookbook 6.1](cookbook.md#example-sharedenvsecrets) |
| Подключить общий ConfigMap в несколько контейнеров | `sharedEnvConfigMaps` + `apps-configmaps` | [Cookbook 6.2](cookbook.md#example-sharedenvconfigmaps) |

Порядок и приоритет подробно:
- [Cookbook 6.3](cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars)
- [Architecture: env order](architecture.md#arch-container-env-order)

## 4. Когда нужен custom renderer

Используйте custom renderer только если встроенные `apps-*` не покрывают ваш CRD/тип.

| Вопрос | Если "да" | Если "нет" |
|---|---|---|
| Нужен нестандартный `kind`, которого нет в `apps-*`? | Делайте `__GroupVars__.type=<custom-type>` + `define "<custom-type>.render"` | Используйте встроенные `apps-*` |
| Нужен полный контроль над YAML ресурса? | Custom renderer | Встроенные секции обычно проще и безопаснее |

Ссылка: [library-guide.md#param-custom-renderer](library-guide.md#param-custom-renderer)

## 5. Чеклист выбора “по умолчанию” (рекомендуемый путь)

1. Workload: `apps-stateless`.
2. Reuse: `_include: ["apps-stateless-defaultApp"]`.
3. Env: `global.env` + `_default` для env-map.
4. Secrets: сначала `fromSecretsEnvVars` для внешних секретов, `secretEnvVars` для managed внутри библиотеки.
5. Scale: включать HPA только после baseline метрик.

## 6. Анти-паттерны

1. Широкие и пересекающиеся regex в env-ключах.
2. Большие монолитные include-профили без разбиения на роли.
3. Использование custom renderer, когда хватает встроенных `apps-*`.
4. Смешивание инфраструктурного refactor и функциональных изменений в одном MR.

## 7. Куда идти дальше

1. Нужен “первый запуск”: [quickstart.md](quickstart.md).
2. Нужен готовый рецепт: [cookbook.md](cookbook.md).
3. Нужны точные поля и типы: [reference-values.md](reference-values.md).
4. Нужен triage/операционка: [operations.md](operations.md).

Навигация: [Наверх](#top)
