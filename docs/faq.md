# Helm Apps Library FAQ
<a id="top"></a>

Частые вопросы по использованию библиотеки.

Быстрая навигация:
- [Старт docs](README.md)
- [Quick Start](quickstart.md)
- [Decision Guide](decision-guide.md)
- [Reference](reference-values.md)
- [Cookbook](cookbook.md)
- [Operations](operations.md)

## 1. Helm или werf?

Оба поддерживаются полностью.

Практика:
- если нужен только рендер/деплой, достаточно Helm;
- если нужен более связный delivery workflow, werf обычно удобнее.

## 2. Почему list в values почти везде запрещены?

Это контракт библиотеки для предсказуемого merge и templating.
Большинство list/map блоков передаются как YAML string (`|`), чтобы рендерить их без неявных преобразований.

Исключения:
- `_include`
- `_include_files`
- `sharedEnvSecrets`
- `sharedEnvConfigMaps`

Также есть opt-in режим:
- `global.validation.allowNativeListsInBuiltInListFields: true`

Он разрешает native YAML lists только для части встроенных Kubernetes list-полей (например `args`, `command`, `ports`, `tolerations`).

Важно:
- строковый формат (`|`) остается базовым и полностью поддерживается;
- merge/includes для native lists сохраняют текущую семантику библиотеки (конкатенация списков), поэтому при `_include` возможны дубли элементов.

## 3. Как задавать окружения правильно?

Через `global.env` + env-map:
- exact env key;
- regex env key;
- `_default`.

Порядок выбора: exact -> regex -> `_default`.

Ссылка: [reference-values.md#param-global-env](reference-values.md#param-global-env)

## 4. Что делать при ошибке ambiguous regex env?

Причина: несколько regex ключей совпали одновременно.

Решение:
1. Убрать пересечение regex.
2. Оставить один regex и `_default`.
3. Проверить рендер с конкретным `global.env`.

Ссылка: [operations.md#43-ошибка-рендера-ambiguous-regex-env](operations.md#43-ошибка-рендера-ambiguous-regex-env)

## 5. Когда нужен `versionKey`?

`versionKey` нужен только если имя app отличается от ключа в `global.releases.<release>`.
Если ключ совпадает с app name, `versionKey` можно не задавать.

Ссылка: [reference-values.md#param-versionkey](reference-values.md#param-versionkey)

## 6. Какой приоритет у `sharedEnvConfigMaps`, `sharedEnvSecrets`, `envFrom`, `secretEnvVars`, `envVars`?

Порядок:
1. Сначала собирается слой `envFrom`: `sharedEnvConfigMaps` -> `sharedEnvSecrets` -> `envFrom` -> auto-secret из `secretEnvVars`.
2. Затем рендерится слой явных `env`-переменных: `envYAML` -> `envVars` -> `env` -> `fromSecretsEnvVars`.

При одинаковом имени переменной явные `env`-переменные имеют приоритет над значениями из `envFrom`.

Ссылки:
- [cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars](cookbook.md#63-порядок-источников-env-sharedenvconfigmapssharedenvsecretsenvfromsecretenvvarsenvvars)

## 7. Почему появляется ошибка `E_TPL_DELIMITERS`?

Эта проверка относится к строкам, которые рендерятся через `fl.value`.

Если у вас literal-контент с `{{ ... }}` (например Grafana dashboards, alert templates, конфиги), включенная проверка может мешать.

Начиная с текущей версии, проверка опциональная:
- `global.validation.validateTplDelimiters: true` — включить строгую проверку;
- по умолчанию `false` — обратная совместимость и меньше ложных падений на literal-данных.
- [architecture.md#arch-container-env-order](architecture.md#arch-container-env-order)

## 7. Можно ли использовать секреты из другого релиза/namespace?

Да, через `fromSecretsEnvVars` и явное указание источника (name/namespace) по поддерживаемому контракту.

Ссылка: [reference-values.md#param-fromsecretsenvvars](reference-values.md#param-fromsecretsenvvars)

## 8. Как добавить собственную сущность без форка библиотеки?

Через custom group:
1. Создать top-level group.
2. Добавить `__GroupVars__.type`.
3. Реализовать `define "<type>.render"` в chart приложения.

Ссылка: [library-guide.md#param-custom-renderer](library-guide.md#param-custom-renderer)

## 9. Как проверить совместимость с целевым Kubernetes?

Минимум:
```bash
helm template my-app .helm --set global.env=prod --kube-version 1.29.0
```

Для legacy:
```bash
helm template my-app .helm --set global.env=prod --kube-version 1.20.15
```

Ссылка: [stability.md](stability.md)

## 10. Где смотреть полный рабочий пример?

- [../tests/.helm/values.yaml](../tests/.helm/values.yaml)
- [../tests/contracts/values.yaml](../tests/contracts/values.yaml)

<a id="flvalue-tpl-errors"></a>
## 11. Почему `fl.value` иногда падает с длинными tpl-ошибками?

`fl.value` рендерит строковые значения через `tpl`, если в строке есть `{{ ... }}`.
Часть ошибок (непарные `{{`/`}}`, тройные скобки) теперь отлавливается заранее и возвращается в коротком формате Helm Apps.

Что проверять:
1. Корректные пары `{{` и `}}`.
2. Нет `{{{` и `}}}`.
3. Внутри шаблона используются доступные переменные контекста (`$.Values`, `$.CurrentApp`, и т.д.).
4. Для env-map задается `global.env`, чтобы выбор значений был предсказуемым.

Практика:
- если нужно вывести литеральные `{{`/`}}`, их нужно экранировать;
- сложные выражения лучше выносить в отдельные helper-шаблоны.

Навигация: [Наверх](#top)
