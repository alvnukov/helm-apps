# Проверка и поиск причины

## Версия, контракт, контекст

Проверь фактически загруженный `charts/helm-apps*/Chart.yaml` или Chart.yaml внутри tgz. `Chart.lock` показывает pin, но лежащий на диске пакет может быть старым. Аннотация `helm-apps/version` в published-package manifests помогает подтвердить renderer; development checkout оставляет placeholder и может её не выводить. Диапазон `~1` и версия embedded happ сами по себе этого не доказывают.

Happ полезен как быстрый путь: overview → app resolve/origin → diff стендов → render/query_manifests. Передай те же env, values_files, set/set_string/set_file, namespace и release name. При несовпадении embedded и consumer версий сравни с реальным Helm; `renderer="helm"` не заменяет проверку фактически выбранной зависимости. Если happ недоступен, используй installed library templates и реальный render.

В checkout библиотечной версии ищи поле в схеме, соответствующий renderer/helper и contract fixture. Схема описывает допустимый input; templates — исполнение; contracts — требуемый результат. Docs удобны для семантики, но расхождения необходимо явно разбирать.

Известные несовпадения документации 1.10.1, которые нельзя переносить в values:

- `docs/reference-values.md`, file includes: путь назван относительным values.yaml. Фактически это `$.Files.Get` от chart.
- Пример merge D в reference утверждает production=4, хотя более поздний профиль задаёт production=2. Фактический merge даёт 2.
- FAQ утверждает concat обычных native lists. В actual helper списки атомарны, concat выделен для `_include`.
- В стороннем описании tool встречается `_includeFile`; в библиотеке такой ключ отсутствует. `_include_from` определён внутренним helper, но init автоматически его не запускает.
- Список `define` может включать текст комментариев, например `fl.percentage`. Сверяй исполняемый template и probe.

Описанные в остальных references варианты подтверждены actual source и render, а не этими ошибочными формулировками.

## Минимальный цикл consumer values

Сначала запиши гипотезу: «production.api имеет 6 replicas, admin остаётся 3; после canary overlay tag api1.29, конфиг слушает порт8080». Проверяй наблюдаемое поведение, а не совпадение входных строк.

```sh
helm lint .helm -f deploy/production.yaml --set global.env=production
helm template demo .helm -f deploy/production.yaml \
  --set global.env=production --namespace demo --kube-version 1.29.0
```

Рендери изменённые env, `_default` fallback и релевантный regex env. Используй реальный release/namespace/target version проекта. Для off-line werf modern cluster явно задай `global.compat.kubeVersion`: offline capabilities могут сообщить1.20. Schema/strict lint не заменяет выходную Kubernetes schema.

При рефакторинге сначала сохрани исходные manifests, после правки сравни. Отсортируй объекты по apiVersion/kind/namespace/name, сравни semantic fields. Исключай только известные непостоянные поля проекта; не выкидывай всё metadata/spec. `randomName`/`alwaysRestart` могут давать intentional differences.

Проверь существенные связки:

- enabled workloads и child resources; не появился лишний app из release matrix;
- image tags, namespace, labels и service selectors;
- числовые ports/replicas и boolean flags имеют правильный тип;
- env string values, secrets/configmap references, volume+mount pairs;
- YAML/JSON **внутри** ConfigMap data тоже разбирается и соответствует стенду;
- требуемые CRD присутствуют в настоящем окружении, raw extras допустимы его API.

При наличии project kubeconform/kube-server проверки используй их. `helm template` подтверждает render, schema validator — форму API, server dry-run — принятие данным API server; эти результаты не доказывают работу приложения в runtime. Не запускай apply/deploy без соответствующего поручения.

Строгие flags включай явно для диагностического прогона, если совместимы с исследуемым набором values; defaults consumer сохраняй:

```sh
helm template demo .helm --set global.env=production \
  --set global.validation.strict=true \
  --set global.validation.validateTplDelimiters=true --kube-version 1.29.0
```

## Если изменяешь саму библиотеку

Consumer skill не заменяет правила её repository. Для behavior/schema/examples/contracts изменений обязательны:

```sh
werf helm lint tests/.helm --values tests/.helm/values.yaml
helm template contracts tests/contracts --set global.env=production
```

При изменении API compatibility дополнительно рендеры тестового chart на1.29.0 и1.20.15, соответствующие contract/API gate проверки из AGENTS.md. Обнови затронутые schema/examples/contracts/docs/CI, сохраняя validation coverage. Сначала убедись, что dependency в tests chart — новая библиотека, не оставшийся старый tgz.

Проверяй lint diagnostics, не только exit code. На werf2.77.2 первая обязательная команда без env выводит `E_ENV_REQUIRED` как INFO, но возвращает0. Дополнительный `werf helm lint tests/.helm --values tests/.helm/values.yaml --env prod` даёт чистый lint. Это повод явно выбрать стенд для meaningful проверки, а не считать нулевой exit доказательством корректного env-resolution.

## Диагностика по последнему верному слою

| Симптом | Сначала проверить |
|---|---|
| Поле неожиданно наследуется | Helm overlays → файл узла → file profiles → named profiles → local field; null может открыть inheritance |
| Выводит map/пустую строку | Direct Values вместо fl.value; map/list вместо scalar; env без fallback |
| Вложенный tpl остаётся текстом | Files.Get без tpl; native list passthrough; structural leaf без _default |
| false включает branch | Проверка непустой строки include вместо fl.isTrue |
| nil pointer/не то имя | `.` relative scope; фаза установки CurrentApp/Container/ParentApp |
| app исчезла | Actual enabled/env, group type, release versionKey, file packaging |
| Шаблон проходит, cluster reject | Target API/CRD, numeric types, raw extras, actual server validation |

Для сложного `_include` уменьши до одного app и одного спорного поля; добавляй layers последовательно. При source conflict сначала покажи противоречие, затем используй локальный probe с правильной версией, не выбирай трактовку молча.

## Проверка поставляемого примера

```sh
ruby scripts/verify_examples.rb /path/to/helm-apps/charts/helm-apps
```

Требуются Ruby со стандартной YAML library и Helm. Скрипт копирует assets и библиотеку в temporary directory; сеть, кластер, исходный chart не меняются. Проверяет dev,production,stage-regex; overlays иCLI priority; env-only override с сохранением default ref; child context/typed config; selective release; custom renderer/boolean false; ожидаемые ошибки ambiguous regex/value ref. Версия библиотечного chart должна совпадать с pin примера. Это проверка semantics примера, не универсальный suite всех Kubernetes kinds.

Для werf-проекта используй его штатный render с выбранным env, например `werf render --env production` с остальными project flags. Это другой путь и источник service values. Проверенный `werf helm template` v2.77.2 внедряет stub env с пустым значением даже поверх `--set global.env`; он не принимает `--env`. Поэтому подстановка этой команды вместо `helm template` не эквивалентна обычному consumer render. Установленный werf и его help определяют допустимые команды; перенос с другой major версии требует сверки.
