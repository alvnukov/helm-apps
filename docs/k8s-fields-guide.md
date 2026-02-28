# Kubernetes-поля в Helm Apps (для разработчиков)

Этот документ объясняет kubernetes-поля, которые часто используются в `values.yaml` библиотеки helm-apps.
Фокус: что это дает приложению и когда это трогать.

## image
<a id="image"></a>

Что это: образ контейнера (`name`, `staticTag`).

Когда менять:
- новый релиз приложения;
- откат на прошлую версию;
- переход на другой registry/image.

## command and args
<a id="command-and-args"></a>

Что это: команда запуска контейнера и аргументы.

Когда менять:
- нужно переопределить entrypoint;
- нужно передать флаги приложению.

## probes (liveness, readiness, startup)
<a id="probes-liveness-readiness-startup"></a>

Что это:
- `readinessProbe`: готов ли pod принимать трафик;
- `livenessProbe`: нужно ли перезапустить контейнер;
- `startupProbe`: защита для долгого старта.

Практика:
- почти всегда начинайте с `readinessProbe`;
- `livenessProbe` добавляйте только когда уверены, что health endpoint надежный;
- для тяжёлого старта используйте `startupProbe`.

## securityContext
<a id="security-context"></a>

Что это: настройки безопасности процесса (`runAsNonRoot`, `runAsUser`, capabilities, readOnlyRootFilesystem).

Зачем:
- снижает риск эскалации прав;
- помогает пройти security-политики кластера.

## affinity, tolerations, nodeSelector
<a id="affinity-tolerations-nodeselector"></a>

Что это:
- `nodeSelector`: простой выбор нод по label;
- `affinity`: гибкие правила размещения;
- `tolerations`: допуск на tainted-ноды.

Когда менять:
- отделить прод от системных нод;
- разнести реплики по разным хостам/зонам;
- посадить сервис на специализированные ноды.

## volumes and volumeMounts
<a id="volumes-and-volumemounts"></a>

Что это:
- `volumes`: источники данных (emptyDir, secret, configMap, pvc и т.д.);
- `volumeMounts`: куда эти данные монтируются в контейнер.

Частые случаи:
- конфиги в `/etc/...`;
- сертификаты/секреты в файловом виде;
- временные файлы через `emptyDir`.

## lifecycle hooks
<a id="lifecycle-hooks"></a>

Что это: хуки `postStart`/`preStop` у контейнера.

Когда применять:
- graceful shutdown (`preStop`);
- одноразовая инициализация при старте (`postStart`).

## envFrom
<a id="envfrom"></a>

Что это: массовый импорт переменных из ConfigMap/Secret.

Важный момент:
- удобно для shared env-наборов;
- может конфликтовать по именам env-переменных — проверяйте порядок и итог.

## ports
<a id="ports"></a>

Что это: порты контейнера/сервиса.

Когда менять:
- меняется порт приложения;
- добавляется второй протокол/порт;
- нужно связать service port и container port.

## Как читать это вместе с helper-полями

- `envVars` / `secretEnvVars` / `sharedEnv*` — это удобные helper-слои библиотеки вокруг Kubernetes env-механики.
- `configFiles*` — helper-слой для ConfigMap/Secret + mount.
- `resources` — helper для CPU/Memory лимитов и запросов.

Смотри также:
- [reference-values.md](reference-values.md)
- [cookbook.md](cookbook.md)
