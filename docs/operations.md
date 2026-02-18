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

## 1. Operational Mindset

При инцидентах действуйте в порядке:
1. Подтвердить симптом (что именно сломано).
2. Локализовать слой (schema -> render -> apply -> runtime).
3. Найти минимальный diff, который вызвал проблему.
4. Восстановить сервис (rollback/hotfix).
5. Зафиксировать постоянное исправление (include/profile/schema/tests).

## 2. Быстрый triage по слоям

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
- `fail` из шаблонов `apps-stateless`/`apps-stateful`/`apps-jobs`/`apps-cronjobs`.

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

Навигация: [Наверх](#top)
