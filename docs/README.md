# Документация Helm Apps Library

Единая точка входа в документацию библиотеки.
Цель: быстро привести пользователя от задачи к корректному `values.yaml` и предсказуемому рендеру.

## Рекомендуемый маршрут

1. Получить первый успешный рендер: [quickstart.md](quickstart.md)
2. Выбрать правильный путь под задачу: [decision-guide.md](decision-guide.md)
3. Взять рабочий рецепт под задачу: [cookbook.md](cookbook.md)
4. Уточнить параметры и типы: [reference-values.md](reference-values.md)
5. Найти параметр по индексу: [parameter-index.md](parameter-index.md)
6. Понять концепцию и границы контрактов: [library-guide.md](library-guide.md)
7. Посмотреть архитектуру рендера и приоритеты merge/env: [architecture.md](architecture.md)
8. Свериться с операционными чеклистами: [operations.md](operations.md)

## Навигация по ролям

| Роль | Что читать сначала | Что читать затем |
|---|---|---|
| Разработчик сервиса | [quickstart.md](quickstart.md), [decision-guide.md](decision-guide.md) | [cookbook.md](cookbook.md), [reference-values.md](reference-values.md) |
| DevOps / Platform Engineer | [library-guide.md](library-guide.md), [architecture.md](architecture.md) | [reference-values.md](reference-values.md), [operations.md](operations.md) |
| Ревьюер MR | [reference-values.md](reference-values.md), [parameter-index.md](parameter-index.md) | [use-case-map.md](use-case-map.md), [operations.md](operations.md) |
| On-call / Incident | [operations.md](operations.md) | [faq.md](faq.md), [reference-values.md](reference-values.md) |

## Навигация по задачам

| Задача | Куда идти |
|---|---|
| Быстро поднять первый сервис | [quickstart.md](quickstart.md) |
| Выбрать подходящий путь без экспериментов | [decision-guide.md](decision-guide.md) |
| Поднять новый API сервис | [cookbook.md#example-basic-api](cookbook.md#example-basic-api) |
| Настроить Ingress/TLS | [cookbook.md#example-ingress-tls](cookbook.md#example-ingress-tls) |
| Подключить shared env из Secret/ConfigMap | [cookbook.md#61-общие-secret-через-sharedenvsecrets](cookbook.md#61-общие-secret-через-sharedenvsecrets), [cookbook.md#62-общие-configmap-через-sharedenvconfigmaps](cookbook.md#62-общие-configmap-через-sharedenvconfigmaps) |
| Быстро понять kubernetes-поля (для разработчика) | [k8s-fields-guide.md](k8s-fields-guide.md) |
| Понять порядок приоритетов env | [cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars](cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars) |
| Включить release matrix | [reference-values.md#param-global-deploy](reference-values.md#param-global-deploy) |
| Сделать переиспользование через include-профили | [reference-values.md#param-global-includes](reference-values.md#param-global-includes), [../README.md#example-global-includes-merge](../README.md#example-global-includes-merge) |
| Добавить custom renderer | [library-guide.md#param-custom-renderer](library-guide.md#param-custom-renderer) |
| Разобраться с ошибкой рендера | [operations.md](operations.md), [faq.md](faq.md) |

## Карта документов

- Быстрый путь до первого результата: [quickstart.md](quickstart.md)
- Гайд выбора подхода: [decision-guide.md](decision-guide.md)
- Концепция и принципы: [library-guide.md](library-guide.md)
- Архитектура рендера и приоритеты: [architecture.md](architecture.md)
- Полный референс параметров: [reference-values.md](reference-values.md)
- Индекс параметров: [parameter-index.md](parameter-index.md)
- Карта use-cases: [use-case-map.md](use-case-map.md)
- Практические рецепты: [cookbook.md](cookbook.md)
- Kubernetes-поля простым языком (RU): [k8s-fields-guide.md](k8s-fields-guide.md)
- Kubernetes fields quick guide (EN): [k8s-fields-guide.en.md](k8s-fields-guide.en.md)
- Эксплуатация и triage: [operations.md](operations.md)
- Вопросы и ответы: [faq.md](faq.md)
- Термины: [glossary.md](glossary.md)
- Стабильность и модель гарантий: [stability.md](stability.md)

## Практические артефакты

- Полный рабочий пример values: [../tests/.helm/values.yaml](../tests/.helm/values.yaml)
- Контрактные кейсы по сущностям: [../tests/contracts/values.yaml](../tests/contracts/values.yaml)
- JSON schema values: [../tests/.helm/values.schema.json](../tests/.helm/values.schema.json)

## Минимальный локальный чек

```bash
helm dependency update .helm
helm lint .helm
helm template my-app .helm --set global.env=prod
```
