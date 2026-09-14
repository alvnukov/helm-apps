# Helm Apps Library Operations Playbook
<a id="top"></a>

Документ для эксплуатации и поддержки деплоев на `helm-apps`:
- как быстро диагностировать проблемы;
- как локализовать источник ошибки;
- какие команды и чеклисты использовать в CI/CD и при релизах;
- как откатываться безопасно.

Быстрая навигация:
- [Старт docs](README.md)
- [Quick Start](quickstart.md)
- [Decision Guide](decision-guide.md)
- [Parameter Index](parameter-index.md)
- [Reference](reference-values.md)
- [Architecture](architecture.md)
- [FAQ](faq.md)

Оглавление:
- [2. Быстрый triage](#2-быстрый-triage-по-слоям)
- [3. Команды диагностики](#3-стандартные-команды-диагностики)
- [4. Частые ошибки](#4-частые-ошибки-и-что-делать)
- [5. Чеклист перед merge](#5-чеклист-изменения-values-перед-merge)
- [6. Чеклист релиза](#6-чеклист-релиза)
- [7. Rollback стратегия](#7-rollback-стратегия)
- [12. Kubernetes API compatibility](#kubernetes-api-compatibility)

## 1. Operational Mindset

При инцидентах действуйте в порядке:
1. Подтвердить симптом (что именно сломано).
2. Локализовать слой (schema -> render -> apply -> runtime).
3. Найти минимальный diff, который вызвал проблему.
4. Восстановить сервис (rollback/hotfix).
5. Зафиксировать постоянное исправление (include/profile/schema/tests).

## 2. Быстрый triage по слоям

### 2.0 Формат ошибок Helm Apps

Новые ошибки библиотеки имеют унифицированный формат:

`[helm-apps:<CODE>] <message> | path=<values-path> | hint=<what-to-do> | docs=<doc-link>`

Что это дает:
1. `CODE` позволяет быстро найти класс проблемы.
2. `path` сразу указывает место в `values.yaml`.
3. `hint` дает минимальное действие для исправления.
4. `docs` ведет в релевантный раздел документации.

### 2.1 Layer 1: Values/Schema

Признаки:
- ошибки валидации `values`;
- не тот тип поля;
- пропущены обязательные ключи.

Проверки:

```bash
helm lint .helm
```

Для репозитория библиотеки:

```bash
helm lint tests/.helm --values tests/.helm/values.yaml
```

### 2.2 Layer 2: Render

Признаки:
- шаблоны не рендерятся;
- ошибки `include`/`tpl`/`required`/`fail`;
- неоднозначный env regex.

Проверки:

```bash
helm template my-app .helm --set global.env=prod
```

Если рендер падает:
- ищите в тексте ошибки полный `CurrentPath` (путь до проблемного блока);
- сверяйте тип/структуру поля с `docs/reference-values.md`;
- проверяйте merge include-блоков.

### 2.3 Layer 3: Apply/Release

Признаки:
- рендер успешен, но релиз не применился;
- ошибки Kubernetes API validation;
- forbidden/unauthorized по RBAC.

Проверки:
- события namespace;
- статус rollout;
- актуальность CRD (для cert-manager/Deckhouse/Strimzi).

### 2.4 Layer 4: Runtime

Признаки:
- pod crashloop;
- readiness/liveness failures;
- нет трафика через ingress/service;
- HPA/VPA не работают как ожидается.

Проверки:
- pod logs/describe;
- service endpoints;
- ingress controller events;
- метрики HPA/VPA.

Навигация: [Наверх](#top)

## 3. Стандартные команды диагностики

### 3.1 Helm

```bash
helm dependency update .helm
helm lint .helm
helm template my-app .helm --set global.env=prod
```

### 3.2 Kubernetes runtime

```bash
kubectl -n <ns> get deploy,sts,job,cronjob,svc,ing,pdb,hpa,vpa
kubectl -n <ns> get pods
kubectl -n <ns> describe pod <pod>
kubectl -n <ns> logs <pod> -c <container>
kubectl -n <ns> get events --sort-by=.metadata.creationTimestamp
```

### 3.3 Service/Ingress debug

```bash
kubectl -n <ns> get endpoints <service-name>
kubectl -n <ns> describe ingress <ingress-name>
```

Навигация: [Наверх](#top)

## 4. Частые ошибки и что делать

## 4.1 Ошибка schema: `Invalid type`

Причина:
- передан map/list вместо строки YAML (или наоборот);
- env-map там, где ожидался plain scalar.

Действия:
1. Проверить поле в `docs/reference-values.md`.
2. Сверить пример в `docs/cookbook.md`.
3. Повторно запустить `helm lint`.

## 4.2 Ошибка рендера: `__GroupVars__ is required`

Причина:
- top-level custom group без `__GroupVars__`;
- schema трактует ключ как custom group.

Действия:
1. Если это custom group, добавить:
```yaml
__GroupVars__:
  type: apps-stateless
```
2. Если это служебный ключ/секция, убедиться, что он описан в schema.

## 4.3 Ошибка рендера: ambiguous regex env

Причина:
- несколько regex-ключей окружений совпали одновременно.

Действия:
1. Убрать пересечение regex.
2. Оставить один явный env-override и `_default`.

## 4.4 Включен app, но не заданы контейнеры

Признак:
- `fail` из шаблонов `apps-stateless`/`apps-stateful`/`apps-daemonsets`/`apps-jobs`/`apps-cronjobs`.

Действия:
1. Добавить `containers`.
2. Либо временно выключить ресурс `enabled: false`.

## 4.5 Service есть, но трафика нет

Причины:
- selector не совпадает с labels pod;
- нет endpoints;
- порт не совпадает (`targetPort` vs container port).

Действия:
1. `kubectl get endpoints`.
2. Сверить selector и labels.
3. Проверить контейнерные порты.

## 4.6 Ingress есть, но 404/502

Причины:
- неверный backend service/port;
- ingress class mismatch;
- TLS secret отсутствует.

Действия:
1. `kubectl describe ingress`.
2. Проверить `ingressClassName`/`class`.
3. Проверить наличие секрета и сертификата.

## 4.7 HPA не скейлит

Причины:
- невалидные metrics;
- отсутствуют источники метрик;
- min/max реплики блокируют ожидаемое поведение.

Действия:
1. Проверить объект HPA и его conditions.
2. Сверить `metrics` и `customMetricResources`.
3. Проверить доступность metrics API.

## 4.8 VPA не влияет на pods

Причины:
- `updateMode: Off`;
- конфликт ожиданий между HPA и VPA;
- ресурс применен, но policy не задает нужное поведение.

Действия:
1. Проверить `updateMode`.
2. Проверить policy.
3. Согласовать autoscaling стратегию.

Навигация: [Reference](reference-values.md) | [Parameter Index](parameter-index.md) | [Наверх](#top)

## 5. Чеклист изменения values перед merge

1. Изменения проходят schema (`helm lint`).
2. Изменения рендерятся в target env (`helm template ... --set global.env=<env>`).
3. Проверены include-конфликты и приоритет override.
4. Для env-ключей нет неоднозначных regex.
5. Для ingress/service проверены имена backend и порты.
6. Для секретов исключены plaintext утечки в git (используйте `secret-values` или внешние хранилища).
7. Для HPA/VPA согласованы min/max/updateMode и metrics.

Навигация: [Наверх](#top)

## 6. Чеклист релиза

1. Подтянуты зависимости чарта.
2. Отрендерен итоговый манифест для target env.
3. Нет неожиданных изменений в критичных ресурсах:
- Service selectors;
- Ingress host/path/tls;
- Stateful PVC/retention settings;
- ServiceAccount/RBAC.
4. Подготовлен rollback-план.

Навигация: [Наверх](#top)

## 7. Rollback стратегия

При регрессии:
1. Откатить `values` к последнему рабочему коммиту.
2. Повторить рендер и деплой.
3. Если проблема в include-profile, зафиксировать hotfix в профиле.

Рекомендации:
- держите small-batch изменения в values;
- не смешивайте в одном MR массовый refactor и функциональные изменения.

Навигация: [Наверх](#top)

## 8. Incident response шаблон

Минимальный протокол:
1. Time started.
2. Затронутые сервисы/окружения.
3. Последний измененный commit в values/include.
4. Симптом/алерт.
5. Layer диагностики (schema/render/apply/runtime).
6. Временное восстановление (rollback/hotfix).
7. Root cause.
8. Permanent fix.
9. Action items.

## 9. Hardening practices

1. Обязательный `helm lint` + `helm template` в CI.
2. Обязательный code-review для include-профилей.
3. Запрет на “широкие” regex для env без необходимости.
4. Разделение common include-профилей по доменам:
- compute;
- networking;
- security;
- autoscaling.
5. Документирование нестандартных hooks рядом с группой.

## 10. Сопровождение schema

При добавлении нового поля/ресурса в библиотеку:
1. Обновить `tests/.helm/values.schema.json`.
2. Добавить пример в `tests/.helm/values.yaml`.
3. Обновить `docs/reference-values.md`.
4. При необходимости добавить рецепт в `docs/cookbook.md`.

Это защищает от дрейфа между кодом библиотеки, примерами и документацией.

## 11. Полезные артефакты в репозитории

- Полные примеры: `tests/.helm/values.yaml`
- Schema: `tests/.helm/values.schema.json`
- Концепция: `docs/library-guide.md`
- Reference: `docs/reference-values.md`
- Cookbook: `docs/cookbook.md`

## 12. Kubernetes API compatibility
<a id="kubernetes-api-compatibility"></a>

Библиотека рендерит манифест под конкретную версию Kubernetes: выбирает
group/version у объектов, которые переезжали между релизами, и убирает поля,
которых ещё нет в схеме целевого кластера.

### 12.1 Откуда берётся версия кластера

1. `global.compat.kubeVersion` — если задан, побеждает всё остальное.
2. Иначе `.Capabilities.KubeVersion.GitVersion` — то, что сообщает рендерер.

Важная ловушка оффлайн-рендера: `werf render` без подключения к кластеру
подставляет **1.20**, а `helm template` — версию, зашитую в свой бинарь. То есть
`werf render` по умолчанию выдаст `batch/v1beta1`, `policy/v1beta1` и
`autoscaling/v2beta2`, которых уже нет в Kubernetes 1.25/1.26. Если рендер
оффлайн уходит в реальный кластер (артефакт для ревью, diff, GitOps-коммит),
задавайте версию явно:

```yaml
global:
  compat:
    kubeVersion: "1.29"
```

### 12.2 Правило выбора порога

Поле включается с того релиза, в котором оно **появилось в схеме API**, а не с
того, в котором стало GA. Ниже этого релиза манифест невалиден целиком: его
отклонят API-сервер, `kubectl --validate` и admission-вебхуки. На версии и выше
худший случай — закрытый feature gate молча отбросит поле, что безвредно и само
исправляется при обновлении кластера.

Пороги проверены по per-version JSON-схемам Kubernetes (`kubeconform -strict
-kubernetes-version X.Y.Z`), а не по памяти.

### 12.3 Таблица порогов

Group/version (`_apps-api-versions.tpl`):

| Объект | Stable group/version | С какого релиза | Fallback |
| --- | --- | --- | --- |
| CronJob | `batch/v1` | 1.21 | `batch/v1beta1` |
| PodDisruptionBudget | `policy/v1` | 1.21 | `policy/v1beta1` |
| HorizontalPodAutoscaler | `autoscaling/v2` | 1.23 | `autoscaling/v2beta2` |

`KafkaTopic` — это Strimzi, а не Kubernetes: `kafka.strimzi.io/v1beta1` выпилен
из CRD в Strimzi 0.23, поэтому по умолчанию рендерится `v1beta2`, а `v1beta1`
берётся только если кластер его всё ещё отдаёт.

Поля (`_apps-compat.tpl`):

| Область | Поле | С какого релиза |
| --- | --- | --- |
| Service | `allocateLoadBalancerNodePorts`, `clusterIPs`, `ipFamilies`, `ipFamilyPolicy` | 1.20 |
| Service | `internalTrafficPolicy`, `loadBalancerClass` | 1.21 |
| Service | `trafficDistribution` | 1.30 |
| StatefulSet | `minReadySeconds` | 1.22 |
| StatefulSet | `persistentVolumeClaimRetentionPolicy` | 1.23 |
| StatefulSet | `ordinals` | 1.26 |
| PodDisruptionBudget | `unhealthyPodEvictionPolicy` | 1.26 |
| CronJob | `timeZone` | 1.24 |
| Job | `completionMode`, `suspend` | 1.21 |
| Job | `podFailurePolicy` | 1.25 |
| Job | `backoffLimitPerIndex`, `maxFailedIndexes`, `podReplacementPolicy` | 1.28 |
| Job | `managedBy`, `successPolicy` | 1.30 |
| PodSpec | `setHostnameAsFQDN` | 1.20 |
| PodSpec | `hostUsers` | 1.25 |
| PodSpec | `schedulingGates` | 1.26 |
| PodSpec | `resourceClaims` | 1.31 |
| Container | `resizePolicy` | 1.27 |
| Container | `restartPolicy` (native sidecar) | 1.28 |

`StatefulSet.spec.progressDeadlineSeconds` и `DaemonSet.spec.replicas`/`strategy`
не существуют ни в одной версии API и удаляются всегда.

`CronJobSpec.suspend` не ограничивается: он есть с самого `batch/v1beta1`.
Порог 1.21 относится только к `JobSpec.suspend`.

### 12.4 Raw escape hatches не нормализуются

`extraSpec`, `podSpecExtra`, `extraFields` и `jobTemplateExtraSpec` проходят
через `apps-compat.renderRaw` **как есть**: версия кластера на них не влияет.
Это сделано намеренно — это аварийный выход для полей, которых библиотека ещё не
знает. Отвечает за совместимость такого блока тот, кто его написал.

### 12.5 Как это проверяется

- `scripts/verify-kube-gates.rb --file FILE --kube-version X.Y.Z` — таблица
  порогов как исполняемая проверка: поле обязано присутствовать на своей версии
  и выше и отсутствовать ниже.
- `scripts/check-contracts.sh` рендерит `tests/contracts` на каждой граничной
  версии и прогоняет через этот скрипт.
- `scripts/ci-local.sh --api` дополнительно валидирует каждый рендер через
  `kubeconform -strict` против схемы соответствующего релиза.
- Матрица `kube-compatibility-matrix` в `.github/workflows/ci.yml` делает то же
  самое в CI.

Если добавляете поле: найдите релиз по схемам (`kubeconform -strict
-kubernetes-version X.Y.Z` на минимальном манифесте с этим полем), добавьте
`apps-compat.pruneBelow` в соответствующий нормализатор, строку в таблицу выше,
запись в `GATES` в `scripts/verify-kube-gates.rb` и фикстуру в
`tests/contracts/values.yaml`.

Навигация: [Наверх](#top)
