# Документация Helm Apps Library

Этот файл — точка входа в документацию.
Если открываете docs впервые, начните отсюда.

Примечание: библиотека полностью поддерживает `Helm` и совместима с `werf`.  
На практике `werf` часто удобнее для продуктовых команд, потому что он объединяет рендер и delivery-процесс в один workflow.  
При этом все сценарии библиотеки доступны через чистый `Helm`.

## Быстрый маршрут (15 минут)

1. Прочитать концепцию и зачем библиотека нужна: `docs/library-guide.md`
2. Взять готовый шаблон под свой сценарий: `docs/cookbook.md`
3. Сверить поля и типы перед merge: `docs/reference-values.md`
4. Проверить values по schema: `tests/.helm/values.schema.json`
5. Сравнить с рабочими примерами: `tests/.helm/values.yaml`

## Как читать документацию по роли

### Разработчик сервиса

1. `docs/cookbook.md`
2. `docs/reference-values.md`
3. `docs/operations.md` (разделы triage и частые ошибки)

### DevOps / Platform Engineer

1. `docs/library-guide.md`
2. `docs/reference-values.md`
3. `docs/operations.md`

### Ревьюер MR с изменениями `.helm/values.yaml`

1. `docs/reference-values.md`
2. `docs/operations.md` (чеклисты merge/release)
3. `tests/.helm/values.schema.json`

## Карта документов

- Архитектура и принципы: `docs/library-guide.md`
- Полный справочник полей: `docs/reference-values.md`
- Готовые практические рецепты: `docs/cookbook.md`
- Эксплуатация, triage, rollback: `docs/operations.md`
- Краткие правила по helper-паттернам: `docs/usage.md`
- Полный рабочий пример values: `tests/.helm/values.yaml`
- Schema валидации values: `tests/.helm/values.schema.json`

## Минимальный командный чеклист

```bash
helm dependency update .helm
helm lint .helm
helm template my-app .helm --set global.env=prod
```
