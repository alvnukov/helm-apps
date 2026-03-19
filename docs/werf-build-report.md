# werf Build Report -> Chart Values

Эта страница описывает отдельный сценарий:
- image уже собраны `werf`;
- chart деплоится отдельно;
- нужно передать в библиотеку те же image refs через values.

## Когда это нужно

Сценарий типичен для:
- shared chart, который переиспользуется в нескольких репозиториях;
- nested chart, куда нужно протащить image refs через `global`;
- CI/CD, где `werf build` выполняется отдельно от `helm upgrade`.

## Контракт библиотеки

Библиотека понимает следующий values overlay:

```yaml
global:
  werfReport:
    image:
      backend: registry.example/project/backend:tag
      frontend: registry.example/project/frontend:tag
```

Важно:
- `global.werfReport.image` используется только как последний fallback;
- `image.repository` влияет только на image refs, которые библиотека собирает сама из `image.name` + tag;
- lookup в `global.werfReport.image` по-прежнему идет по ключу `image.name`;
- текущий порядок резолва не меняется:
  - `image.staticTag`
  - `CurrentAppVersion`
  - `Values.werf.image`
  - `Values.global.werfReport.image`
- отсутствие `global.werfReport` не роняет рендер.

## Шаг 1. Сохранить build report

```bash
werf build --save-build-report --build-report-path .werf-build-report.json --repo REPO
```

Ниже используется JSON report, потому что из него проще собрать values overlay без хардкода имён image.

## Шаг 2A. Преобразовать report через `zq`

```bash
zq --output-format=yaml \
  '{"global":{"werfReport":{"image":(.Images | with_entries(.value = .value.DockerImageName))}}}' \
  .werf-build-report.json > werf-report-values.yaml
```

## Шаг 2B. Преобразовать report через `jq`

```bash
jq '
  {
    global: {
      werfReport: {
        image: (
          .Images
          | with_entries(.value = .value.DockerImageName)
        )
      }
    }
  }
' .werf-build-report.json > werf-report-values.json
```

Оба варианта дают один и тот же shape:

```yaml
global:
  werfReport:
    image:
      backend: registry.example/project/backend:tag
```

## Шаг 3. Передать overlay в chart

YAML-вариант:

```bash
helm upgrade --install my-app ./chart \
  -f values.yaml \
  -f werf-report-values.yaml
```

JSON-вариант:

```bash
helm upgrade --install my-app ./chart \
  -f values.yaml \
  -f werf-report-values.json
```

Helm принимает и YAML, и JSON values files, поэтому дополнительная конвертация не нужна.

## Ограничения

- `global.werfReport.image` не override-ит `image.staticTag`, `CurrentAppVersion` и локальный `Values.werf.image`;
- `image.repository` не префиксует `global.werfReport.image` и `Values.werf.image`; эти источники считаются уже готовыми full refs;
- если в chart уже задан более приоритетный источник image, будет использован он;
- для live `werf render/deploy` по-прежнему главным остаётся локальный `Values.werf.image`.
