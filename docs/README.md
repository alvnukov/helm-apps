# Документация Helm Apps Library

Этот файл — точка входа в документацию.
Если открываете docs впервые, начните отсюда.

Примечание: библиотека полностью поддерживает `Helm` и совместима с `werf`.  
На практике `werf` часто удобнее для продуктовых команд, потому что он объединяет рендер и delivery-процесс в один workflow.  
При этом все сценарии библиотеки доступны через чистый `Helm`.

## Быстрый маршрут (15 минут)

1. Прочитать концепцию и зачем библиотека нужна: [docs/library-guide.md](library-guide.md)
2. Взять готовый шаблон под свой сценарий: [docs/cookbook.md](cookbook.md)
3. Сверить поля и типы перед merge: [docs/reference-values.md](reference-values.md)
4. Быстро найти нужный параметр и пример: [docs/parameter-index.md](parameter-index.md)
5. Найти решение по задаче: [docs/use-case-map.md](use-case-map.md)
6. Проверить values по schema: [tests/.helm/values.schema.json](../tests/.helm/values.schema.json)
7. Сравнить с рабочими примерами: [tests/.helm/values.yaml](../tests/.helm/values.yaml)
8. Понять модель надежности и уровень гарантий: [docs/stability.md](stability.md)

## Как читать документацию по роли

### Разработчик сервиса

1. `docs/cookbook.md`
2. `docs/reference-values.md`
3. `docs/parameter-index.md` (быстрый переход по параметрам)
4. `docs/use-case-map.md` (карта решений по задачам)
5. `docs/operations.md` (разделы triage и частые ошибки)

### DevOps / Platform Engineer

1. `docs/library-guide.md`
2. `docs/reference-values.md`
3. `docs/parameter-index.md`
4. `docs/use-case-map.md`
5. `docs/operations.md`

### Ревьюер MR с изменениями `.helm/values.yaml`

1. `docs/reference-values.md`
2. `docs/parameter-index.md`
3. `docs/use-case-map.md`
4. `docs/operations.md` (чеклисты merge/release)
5. `tests/.helm/values.schema.json`

## Карта документов

- Архитектура и принципы: [docs/library-guide.md](library-guide.md)
- Полный справочник полей: [docs/reference-values.md](reference-values.md)
- Индекс параметров с примерами: [docs/parameter-index.md](parameter-index.md)
- Карта use-cases: [docs/use-case-map.md](use-case-map.md)
- Стабильность и модель гарантий: [docs/stability.md](stability.md)
- Готовые практические рецепты: [docs/cookbook.md](cookbook.md)
- Эксплуатация, triage, rollback: [docs/operations.md](operations.md)
- Краткие правила по helper-паттернам: [docs/usage.md](usage.md)
- Полный рабочий пример values: [tests/.helm/values.yaml](../tests/.helm/values.yaml)
- Schema валидации values: [tests/.helm/values.schema.json](../tests/.helm/values.schema.json)

## Минимальный командный чеклист

```bash
helm dependency update .helm
helm lint .helm
helm template my-app .helm --set global.env=prod
```
