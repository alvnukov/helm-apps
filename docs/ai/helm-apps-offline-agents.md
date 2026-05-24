# AGENTS.md

This file is the offline operating guide for AI agents using the packaged `helm-apps` Helm library chart.
It is intentionally stored inside the chart so it is available in air-gapped environments where repository docs are missing.

No guide can make an LLM always correct without rendering and validation. The contract here is: do not guess syntax, prefer the patterns below, inspect chart templates when a field is unclear, and verify the result with Helm.

## 1. Scope

Use this file when you need to write or review consumer `values.yaml` for the `helm-apps` library.
The chart is a Helm library chart (`type: library`), so it provides templates and helpers but does not render resources until a consumer chart initializes it.

The consumer chart must include exactly this initialization template:

```yaml
{{- include "apps-utils.init-library" $ }}
```

Do not call individual `apps-*` render templates directly from a consumer chart.

## 2. Offline Source Priority

In a packaged chart, use sources in this order:

1. This `AGENTS.md` for the strict offline operating contract.
2. `docs/ai/helm-apps-capabilities.prompt.md` for compact machine-oriented syntax.
3. `docs/reference-values.md` for parameter semantics and validation flags.
4. `docs/decision-guide.md`, `docs/quickstart.md`, `docs/cookbook.md`, and `docs/faq.md` for examples and common decisions.
5. `templates/_apps-default-values.yaml` for built-in defaults and include profiles.
6. `templates/_apps-*.tpl` for exact renderer behavior.
7. Render output from `helm template` for final confirmation.

If repository-level schema or tests are unavailable in airgap, do not invent missing syntax. Prefer documented patterns here, inspect the packaged templates, and verify with `helm template`.

## 3. Required Work Pattern

For any values change:

1. Identify the resource type and choose the right `apps-*` group.
2. Use an existing pattern from this file before inventing a new shape.
3. Prefer YAML block strings for Kubernetes object/list fragments.
4. Use `global.env` and env-maps for environment-specific values.
5. Render with the target environment before finishing.
6. If Helm renders invalid YAML or a library error, fix the values contract, not the rendered YAML.

Minimum local check for a consumer chart:

```bash
helm lint .helm
helm template release-name .helm --set global.env=prod
```

Use the real chart path and target environment for the project you are editing.

## 4. Top-Level Values Sections

Built-in groups:

- `apps-stateless` - Deployment workloads.
- `apps-stateful` - StatefulSet workloads.
- `apps-jobs` - Job workloads.
- `apps-cronjobs` - CronJob workloads.
- `apps-services` - Service resources.
- `apps-ingresses` - Ingress resources, optional Certificate and DexAuthenticator integration.
- `apps-network-policies` - Kubernetes, Cilium, or Calico network policies.
- `apps-configmaps` - ConfigMap resources.
- `apps-secrets` - Secret resources.
- `apps-pvcs` - PersistentVolumeClaim resources.
- `apps-limit-range` - LimitRange resources.
- `apps-certificates` - cert-manager Certificate resources.
- `apps-dex-clients` - DexClient resources.
- `apps-dex-authenticators` - DexAuthenticator resources.
- `apps-custom-prometheus-rules` - Prometheus rule resources.
- `apps-grafana-dashboards` - GrafanaDashboardDefinition resources.
- `apps-kafka-strimzi` - Strimzi Kafka, KafkaTopic, and related resources.
- `apps-infra` - Deckhouse infra helpers such as NodeUser and NodeGroup.
- `apps-k8s-manifests` - raw Kubernetes manifests managed through the library cycle.
- `apps-service-accounts` - ServiceAccount and RBAC resources.

Other supported top-level keys:

- `global` - environment, reusable include profiles, validation flags, release matrix, common variables.
- `werf` - werf integration values.
- `helm-apps` - internal/service values that may appear in merged values.

Unknown top-level `apps-*` names are valid only for custom groups with `__GroupVars__.type`.

## 5. Common App Map Shape

Most groups use a map of app name to app config:

```yaml
apps-stateless:
  api:
    enabled: true
    name: api
    _include: ["apps-stateless-defaultApp"]
```

Rules:

- App key should start with an alphanumeric character and then use alphanumeric, `_`, `.`, or `-`.
- `enabled: true` renders the app; disabled apps are skipped unless release mode auto-enables them.
- `name` overrides the rendered Kubernetes resource name; otherwise the app key is usually used.
- `_include` is a native YAML list of profile names.
- `__GroupVars__` is reserved for group configuration.
- `__AppType__` can override the renderer for one app inside a custom group.

## 6. Environment Values

Environment is selected through `global.env`:

```yaml
global:
  env: prod
```

Most scalar values can be written directly or as an env-map:

```yaml
apps-stateless:
  api:
    replicas:
      _default: 1
      prod: 3
      "^stage-.*$": 2
```

Resolution order:

1. exact key equal to `global.env`;
2. regex key;
3. `_default`.

Multiple regex matches for one value are an error. For nested structures such as `envYAML` or `configFilesYAML`, use `_default` unless a rendered check proves a narrower env-only form is supported.

## 7. YAML Block String Rule

Default rule: fields that represent Kubernetes maps/lists should be written as YAML block strings.

Correct:

```yaml
ports: |
  - name: http
    containerPort: 80
annotations: |
  prometheus.io/scrape: "true"
tolerations: |
  - key: dedicated
    operator: Equal
    value: api
    effect: NoSchedule
```

Avoid native YAML lists/maps unless the field is a documented exception:

```yaml
ports:
  - name: http
    containerPort: 80
```

Always allowed native YAML lists:

- `_include`
- `_include_files`

Common documented native structures include `envVars` maps and selected nested structures such as `configFilesYAML` / `envYAML`, but Kubernetes list fragments should still default to block strings.

`global.validation.allowNativeListsInBuiltInListFields: true` is an experimental opt-in for selected built-in list fields. Do not rely on it as the default syntax, especially when templated values must become numbers, booleans, or nulls after rendering.

## 8. Includes And Merge

Reusable profiles live in `global._includes` and are attached with `_include`:

```yaml
global:
  _includes:
    profile-base:
      service:
        enabled: true
        ports: |
          - name: http
            port: 80
    profile-prod:
      replicas: 3

apps-stateless:
  api:
    _include: ["profile-base", "profile-prod"]
```

Merge contract:

- map merge is recursive;
- include order matters: later include can override earlier include;
- local app values override included values;
- `_include` chains are concatenated.

Do not change values to depend on unclear merge behavior. Render and inspect the final manifest.

## 9. Workload Groups

Use:

- `apps-stateless` for Deployment.
- `apps-stateful` for StatefulSet.
- `apps-jobs` for Job.
- `apps-cronjobs` for CronJob.

Minimal stateless app:

```yaml
apps-stateless:
  api:
    enabled: true
    replicas: 2
    containers:
      main:
        image:
          name: nginx
          staticTag: "1.27"
        ports: |
          - name: http
            containerPort: 80
    service:
      enabled: true
      ports: |
        - name: http
          port: 80
```

Common workload fields:

- `replicas`
- `containers`
- `initContainers`
- `service`
- `serviceAccount`
- `affinity`
- `tolerations`
- `nodeSelector`
- `topologySpreadConstraints`
- `imagePullSecrets`
- `volumes`
- `podDisruptionBudget`
- `verticalPodAutoscaler`
- `horizontalPodAutoscaler` for stateless workloads
- `priorityClassName`

Jobs and CronJobs also support fields such as `schedule`, `concurrencyPolicy`, `successfulJobsHistoryLimit`, `failedJobsHistoryLimit`, `startingDeadlineSeconds`, `backoffLimit`, `activeDeadlineSeconds`, and `restartPolicy`.

## 10. Container Syntax

Container image:

```yaml
containers:
  main:
    image:
      repository: registry.example.com/team
      name: api
      staticTag: "1.2.3"
```

If `staticTag` is absent and release mode resolves an app version, that version can be used as the image tag.

Typical container keys:

- `image`
- `command`
- `args`
- `workingDir`
- `envVars`
- `secretEnvVars`
- `fromSecretsEnvVars`
- `sharedEnvConfigMaps`
- `sharedEnvSecrets`
- `envFrom`
- `envYAML`
- `resources`
- `configFiles`
- `configFilesYAML`
- `secretConfigFiles`
- `volumeMounts`
- `persistantVolumes`
- `livenessProbe`
- `readinessProbe`
- `startupProbe`
- `lifecycle`
- `securityContext`
- `alwaysRestart`

Environment examples:

```yaml
envVars:
  LOG_LEVEL: info
  FEATURE_FLAG:
    _default: "false"
    prod: "true"

envYAML:
  _default: |
    - name: POD_NAME
      valueFrom:
        fieldRef:
          fieldPath: metadata.name
```

Resources example:

```yaml
resources:
  requests:
    mcpu: 100
    memoryMb: 128
  limits:
    memoryMb: 512
```

## 11. ConfigMaps And Secrets

ConfigMap:

```yaml
apps-configmaps:
  app-config:
    enabled: true
    data: |
      application.yaml: |
        server:
          port: 8080
```

Secret:

```yaml
apps-secrets:
  app-secret:
    enabled: true
    stringData: |
      password: "change-me"
```

Container-generated config files:

```yaml
containers:
  main:
    configFiles:
      app-config:
        mountPath: /etc/app
        data:
          application.yaml: |
            server:
              port: 8080
```

Do not put real secrets into committed values. Use deployment-time secret injection when the project supports it.

## 12. Services

Use workload-local `service` when the Service belongs directly to one workload:

```yaml
apps-stateless:
  api:
    service:
      enabled: true
      ports: |
        - name: http
          port: 80
          targetPort: http
```

Use `apps-services` for standalone or shared Services:

```yaml
apps-services:
  api:
    enabled: true
    selector: |
      app.kubernetes.io/name: api
    ports: |
      - name: http
        port: 80
        targetPort: 8080
```

Common service fields include `type`, `ports`, `selector`, `clusterIP`, `sessionAffinity`, annotations, and labels.

## 13. Ingresses

Ingress example:

```yaml
apps-ingresses:
  api:
    enabled: true
    host: api.example.com
    ingressClassName: nginx
    paths: |
      - path: /
        pathType: Prefix
        backend:
          service:
            name: api
            port:
              number: 80
    tls:
      enabled: true
```

If `tls.enabled=true` and no secret name is provided, the library may generate a Certificate depending on chart settings. `dexAuth` can generate a related DexAuthenticator.

## 14. Network Policies

Select implementation with `type`:

- `kubernetes` -> `networking.k8s.io/v1` + `NetworkPolicy`
- `cilium` -> `cilium.io/v2` + `CiliumNetworkPolicy`
- `calico` -> `projectcalico.org/v3` + `NetworkPolicy`

Kubernetes example:

```yaml
apps-network-policies:
  api:
    enabled: true
    type: kubernetes
    podSelector: |
      matchLabels:
        app.kubernetes.io/name: api
    policyTypes: |
      - Ingress
    ingress: |
      - from:
          - namespaceSelector: {}
```

Do not mix provider-specific fields unless they belong to the selected policy type.

## 15. Service Accounts And RBAC

For new code prefer `apps-service-accounts` over workload-local legacy `serviceAccount.clusterRole`.

Example shape:

```yaml
apps-service-accounts:
  api:
    enabled: true
    name: api
    rules:
      read-pods:
        apiGroups: |
          - ""
        resources: |
          - pods
        verbs: |
          - get
          - list
```

Legacy workload-local shape can still exist for compatibility:

```yaml
serviceAccount:
  enabled: true
  name: api
  clusterRole:
    name: api:read
    rules: |
      - apiGroups: [""]
        resources: ["pods"]
        verbs: ["get", "list"]
```

`global.validation.forbidLegacyServiceAccountClusterRole: true` forbids the legacy path.

## 16. Child Apps

Workload apps may declare related resources under `childApps`.
Allowed child groups:

- `apps-certificates`
- `apps-configmaps`
- `apps-ingresses`
- `apps-k8s-manifests`
- `apps-network-policies`
- `apps-pvcs`
- `apps-secrets`
- `apps-service-accounts`
- `apps-services`

Example:

```yaml
apps-stateless:
  api:
    enabled: true
    containers:
      main:
        image:
          name: nginx
          staticTag: "1.27"
    childApps:
      apps-configmaps:
        runtime-config:
          enabled: true
          name: "{{ $.ParentApp.name }}-config"
          data: |
            parentName: {{ $.ParentApp.name | quote }}
```

Use `$.ParentApp` only inside child app values rendered in parent context.

## 17. PVCs And Certificates

PVC example:

```yaml
apps-pvcs:
  data:
    enabled: true
    storageClassName: fast
    accessModes: |
      - ReadWriteOnce
    resources:
      requests:
        storage: 10Gi
```

Certificate example:

```yaml
apps-certificates:
  api-cert:
    enabled: true
    secretName: api-tls
    dnsNames: |
      - api.example.com
    clusterIssuer: letsencrypt
```

## 18. Raw Kubernetes Manifests

Use `apps-k8s-manifests` only when no dedicated group fits.
Prefer dedicated groups because they carry library conventions and validations.

Typical raw-manifest pattern:

```yaml
apps-k8s-manifests:
  custom-resource:
    enabled: true
    manifest: |
      apiVersion: example.com/v1
      kind: Example
      metadata:
        name: example
      spec:
        enabled: true
```

Verify the exact supported key in `templates/_apps-k8s-manifests.tpl` if this pattern fails.

## 19. Kafka Strimzi

Use `apps-kafka-strimzi` for Strimzi Kafka-related resources.
Common nested areas include `kafka`, `zookeeper`, `topics`, `entityOperator`, `exporter`, and `deckhouseMetrics`.

Because Strimzi CRDs are large and version-sensitive, do not invent nested Kafka fields. Inspect `templates/_apps-kafka-strimzi.tpl` and render against the target cluster version.

## 20. Release Mode

Release matrix mode uses:

- `global.deploy.enabled` as the master switch;
- `global.deploy.release` as release name, scalar or env-map;
- `global.deploy.autoEnableApps` to auto-enable apps with a release version;
- `global.deploy.annotateAllWithRelease` to annotate all rendered resources;
- `global.releases` as `release -> appKey -> tag/version` matrix;
- app-level `versionKey` to override the lookup key.

Example:

```yaml
global:
  deploy:
    enabled: true
    autoEnableApps: true
    release:
      _default: dev-v1
      prod: prod-v1
  releases:
    prod-v1:
      api: "1.4.2"

apps-stateless:
  api:
    containers:
      main:
        image:
          name: api
```

If `image.staticTag` is absent and a release version is found, the resolved app version can become the image tag.

## 21. Custom Groups And Renderers

Custom group backed by a built-in renderer:

```yaml
payment-group:
  __GroupVars__:
    type:
      _default: apps-stateless
      prod: apps-stateful
  api:
    _include: ["apps-stateless-defaultApp"]
```

Per-app renderer override:

```yaml
payment-group:
  __GroupVars__:
    type: apps-stateless
  edge:
    __AppType__: apps-ingresses
```

Custom renderer contract:

1. Set `__GroupVars__.type: <custom-type>`.
2. Define template `"<custom-type>.render"` in the consumer chart.
3. The library calls `include (printf "%s.render" $type) $`.

Renderer context includes `$`, `$.Values`, `$.CurrentApp`, `$.CurrentGroupVars`, `$.CurrentGroup`, `$.CurrentPath`, `$.Release`, `$.Capabilities`, and `$.Files`.

## 22. Validation Flags

Known validation flags:

```yaml
global:
  validation:
    strict: false
    allowNativeListsInBuiltInListFields: false
    forbidLegacyServiceAccountClusterRole: false
    validateTplDelimiters: false
```

Meaning:

- `strict` enables stricter contract checks where implemented; default is false for 1.x compatibility.
- `allowNativeListsInBuiltInListFields` allows selected native lists; experimental opt-in.
- `forbidLegacyServiceAccountClusterRole` rejects workload-local legacy RBAC.
- `validateTplDelimiters` checks balance of `{{` / `}}` and rejects `{{{` / `}}}` in strings processed by `fl.value`.

Do not enable stricter flags in shared defaults unless the project intentionally accepts the compatibility change.

## 23. Templating Rules

Many string values are processed through Helm `tpl`-style rendering. You may reference root values:

```yaml
host: "api.{{ $.Values.global.ci_url }}"
```

For child app context, `$.ParentApp` may be available as shown in the childApps section.

If a templated value must render as a number, boolean, object, or list, prefer YAML block string so Helm can parse the rendered YAML fragment correctly.

## 24. Common Mistakes

Avoid these patterns:

- Using native YAML lists for `ports`, `paths`, `tolerations`, `ingress`, `egress`, or Kubernetes rule arrays without a documented opt-in.
- Forgetting `global.env` when any env-map values are present.
- Using unknown `apps-*` groups without `__GroupVars__.type`.
- Mixing Cilium/Calico fields into `type: kubernetes` network policies.
- Adding `serviceAccount.clusterRole` in new code instead of `apps-service-accounts`.
- Changing include order without checking merged output.
- Fixing rendered YAML by hand instead of fixing values.

## 25. Final Verification

Before delivering values to a user, run the narrowest relevant render:

```bash
helm template release-name .helm --set global.env=<env>
```

For Kubernetes compatibility-sensitive changes, render with explicit versions:

```bash
helm template release-name .helm --set global.env=<env> --kube-version 1.29.0
helm template release-name .helm --set global.env=<env> --kube-version 1.20.15
```

A successful answer should state:

- what values were changed;
- which `apps-*` groups were used;
- which render/lint checks passed;
- any syntax that was inferred from templates instead of this guide.
