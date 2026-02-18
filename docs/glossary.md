# Helm Apps Library Glossary

Краткий словарь терминов библиотеки.

| Термин | Значение |
|---|---|
| `apps-*` | Встроенные группы ресурсов (`apps-stateless`, `apps-ingresses`, `apps-secrets` и т.д.). |
| `group` | Top-level секция в `values.yaml`, содержащая набор приложений. |
| `app` | Элемент внутри группы; обычно соответствует одному workload/resource. |
| `global.env` | Текущее окружение, по которому выбираются env-map значения. |
| env-map | Структура вида `{ _default: ..., production: ..., "^prod-.*$": ... }`. |
| `_default` | Значение по умолчанию в env-map при отсутствии exact/regex match. |
| `global._includes` | Реестр переиспользуемых профилей конфигурации. |
| `_include` | Список include-профилей, применяемых к приложению. |
| include merge | Рекурсивное объединение map-структур профилей и app overrides. |
| `release mode` | Режим `global.deploy` + `global.releases` для централизованного контроля версий приложений. |
| `versionKey` | Ключ app в `global.releases.<release>` (опционален). |
| `custom group` | Пользовательская группа с `__GroupVars__.type`. |
| `custom renderer` | Шаблон `define "<type>.render"`, вызываемый библиотекой при рендере группы. |
| `CurrentApp` | Контекст текущего приложения (`$.CurrentApp`) внутри renderer. |
| `CurrentPath` | Путь до текущего app/group в values, используется в ошибках и диагностике. |
| `sharedEnvSecrets` | Список Secret, подключаемых контейнеру через `envFrom.secretRef`. |
| `sharedEnvConfigMaps` | Список ConfigMap, подключаемых контейнеру через `envFrom.configMapRef`. |
| `contract tests` | Проверки ожидаемого рендера по эталонным контрактным values. |
| `schema validation` | JSON Schema проверка структуры и типов values до рендера. |

## Связанные документы

- [README.md](../README.md)
- [quickstart.md](quickstart.md)
- [decision-guide.md](decision-guide.md)
- [architecture.md](architecture.md)
- [reference-values.md](reference-values.md)
- [cookbook.md](cookbook.md)
- [faq.md](faq.md)
