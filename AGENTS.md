# AGENTS.md

This file is the operating contract for AI agents working with the `helm-apps` Helm library.
It must keep agents from guessing syntax: read the canonical sources first, use the schema-backed contract, and verify rendered output.

No instruction can guarantee perfect answers without verification. Treat this file as a guardrail: if syntax is not confirmed by schema, docs, examples, or templates, do not invent it.

## 1. Required Workflow

For every task involving this library:

1. Classify the task: consumer values, documentation, tests, or library behavior.
2. Read the smallest relevant source set before editing:
   - `docs/ai/helm-apps-capabilities.prompt.md` for machine-oriented syntax summary.
   - `tests/.helm/values.schema.json` for allowed keys and types.
   - `docs/reference-values.md` for parameter semantics.
   - `tests/.helm/values.yaml` and `tests/contracts/values.yaml` for working examples.
   - `charts/helm-apps/templates/` only when changing render behavior.
3. State the concrete hypothesis before changing anything.
4. Make the smallest local change that satisfies the contract.
5. Run the narrowest meaningful checks, plus the mandatory repository checks below when required.
6. If evidence conflicts, stop and report the conflict instead of guessing.

## 2. Consumer Chart Entrypoint

Every consumer chart must initialize the library exactly once from templates:

```yaml
{{- include "apps-utils.init-library" $ }}
```

Do not replace this with a direct include of individual render templates.

## 3. Canonical Values Shape

Top-level `values.yaml` keys are schema-backed. Built-in render groups are:

- `apps-stateless`
- `apps-stateful`
- `apps-jobs`
- `apps-cronjobs`
- `apps-services`
- `apps-ingresses`
- `apps-network-policies`
- `apps-configmaps`
- `apps-secrets`
- `apps-pvcs`
- `apps-limit-range`
- `apps-certificates`
- `apps-dex-clients`
- `apps-dex-authenticators`
- `apps-custom-prometheus-rules`
- `apps-grafana-dashboards`
- `apps-kafka-strimzi`
- `apps-infra`
- `apps-k8s-manifests`
- `apps-service-accounts`

Other schema-backed top-level keys:

- `global`
- `helm-apps`
- `werf`

Unknown top-level `apps-*` keys are allowed only as custom groups with `__GroupVars__.type`; otherwise they must fail under strict validation.

## 4. App Map Syntax

Most built-in groups use this app map shape:

```yaml
apps-stateless:
  api:
    enabled: true
    name: api
    _include: ["apps-stateless-defaultApp"]
```

Rules:

- App keys must match schema app-name rules: start with an alphanumeric character; then use alphanumeric, `_`, `.`, or `-`.
- `__GroupVars__` is reserved for group settings.
- `_include` is a native YAML list of include profile names.
- `__AppType__` may override the renderer for one app inside a custom group.
- Local app values override included values.

## 5. Environment Values

Environment selection is always through `global.env`.
Any env-specific value should use an env-map:

```yaml
global:
  env: prod

apps-stateless:
  api:
    replicas:
      _default: 1
      prod: 3
      "^stage-.*$": 2
```

Resolution order:

1. exact `global.env` key;
2. regex key;
3. `_default`.

Multiple regex matches for the same value are an error. For nested env structures like `envYAML` and `configFilesYAML`, provide `_default` unless the docs prove another shape is valid.

## 6. YAML Block String Rule

Default rule: Kubernetes maps/lists in values should be YAML block strings (`|`), not native YAML lists/maps.

Correct:

```yaml
ports: |
  - name: http
    containerPort: 80
annotations: |
  prometheus.io/scrape: "true"
```

Avoid unless a documented exception applies:

```yaml
ports:
  - name: http
    containerPort: 80
```

Native YAML lists are always allowed for `_include` and `_include_files`. Other native-list allowances are documented exceptions or opt-in behavior; verify them in `docs/reference-values.md`, `docs/faq.md`, or `charts/helm-apps/templates/_apps-compat.tpl` before using them.
If unsure, use a YAML block string.

`global.validation.allowNativeListsInBuiltInListFields: true` is experimental opt-in. It permits selected built-in list fields, but it is not the default contract and does not replace block strings for templated scalar values.

## 7. Includes And Merge

Reusable profiles live under `global._includes` and are attached with `_include`:

```yaml
global:
  _includes:
    profile-base:
      service:
        enabled: true
        ports: |
          - name: http
            port: 80

apps-stateless:
  api:
    _include: ["profile-base"]
```

Merge contract:

- map merge is recursive;
- include order matters, later include overrides earlier include;
- local app values override all includes;
- `_include` chains are concatenated.

Do not change merge semantics without contract tests.

## 8. Workload Syntax

Use `apps-stateless` for `Deployment`, `apps-stateful` for `StatefulSet`, `apps-jobs` for `Job`, and `apps-cronjobs` for `CronJob`.

Common workload shape:

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

Container layer supports `containers` and `initContainers`; use documented keys for image, env, resources, probes, lifecycle, security context, config files, and mounts.
For new RBAC, prefer `apps-service-accounts`; `serviceAccount.clusterRole` is legacy and can be forbidden with `global.validation.forbidLegacyServiceAccountClusterRole: true`.

## 9. Child Apps

Workload apps may define related built-in resources under `childApps`.
Allowed child groups are schema-backed:

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

Use `$.ParentApp` only inside child app values that are rendered in parent context.

## 10. Networking

For services use either standalone `apps-services` or workload-local `service`.
For ingress use `apps-ingresses` with `host`, `paths`, `class`/`ingressClassName`, and optional `tls`/`dexAuth`.

Network policy implementation is selected with `type`:

- `kubernetes` -> `networking.k8s.io/v1` + `NetworkPolicy`
- `cilium` -> `cilium.io/v2` + `CiliumNetworkPolicy`
- `calico` -> `projectcalico.org/v3` + `NetworkPolicy`

Do not mix provider-specific fields unless the selected type supports them.

## 11. Release Mode

Release matrix mode uses:

- `global.deploy.enabled` as the master switch;
- `global.deploy.release` as release name, string or env-map;
- `global.deploy.autoEnableApps` to auto-enable apps with a release version;
- `global.deploy.annotateAllWithRelease` to annotate all rendered resources;
- `global.releases` as `release -> appKey -> tag/version` matrix;
- app-level `versionKey` to override lookup key; fallback is app name.

When `image.staticTag` is absent and a release version is resolved, the library uses `CurrentAppVersion` as image tag and adds release/version annotations according to the release settings.

## 12. Custom Groups And Renderers

Custom group using a built-in renderer:

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

## 13. Validation Flags

Known `global.validation` flags:

- `strict`: opt-in contract validation; default is `false` for 1.x compatibility.
- `allowNativeListsInBuiltInListFields`: experimental opt-in native lists for selected built-in list fields.
- `forbidLegacyServiceAccountClusterRole`: forbids workload-local legacy `serviceAccount.clusterRole`.
- `validateTplDelimiters`: checks balance of `{{`/`}}` and rejects `{{{`/`}}}` in strings processed through `fl.value`.

Do not enable stricter flags in examples or defaults unless the task explicitly asks for that compatibility change.

## 14. Source Of Truth Rules

When syntax is unclear, priority is:

1. `tests/.helm/values.schema.json` for allowed keys/types.
2. `charts/helm-apps/templates/` for actual render behavior.
3. `tests/contracts/` for required behavior.
4. `docs/reference-values.md` and `docs/ai/helm-apps-capabilities.prompt.md` for documented syntax.
5. `README.md` and cookbook examples for onboarding patterns.

If these sources disagree, report the mismatch and do not silently choose the convenient interpretation.

## 15. Editing Scope

If you modify library behavior, update all relevant artifacts:

1. templates in `charts/helm-apps/templates/`;
2. examples in `tests/.helm/values.yaml`;
3. schema in `tests/.helm/values.schema.json`;
4. contract tests in `tests/contracts/`;
5. CI checks in `.github/workflows/ci.yml`;
6. docs and changelog/release notes when user-facing behavior changes.

Do not mix behavior changes with unrelated formatting or documentation cleanup.

## 16. Mandatory Checks Before Final Answer

For library behavior, schema, contract, or examples changes, run:

```bash
werf helm lint tests/.helm --values tests/.helm/values.yaml
helm template contracts tests/contracts --set global.env=production
```

If compatibility behavior changed, also run:

```bash
helm template tests tests/.helm --set global.env=prod --set global._includes.apps-defaults.enabled=true --kube-version 1.29.0
helm template tests tests/.helm --set global.env=prod --set global._includes.apps-defaults.enabled=true --kube-version 1.20.15
```

For AGENTS.md-only changes, at minimum verify that its schema-backed section lists match `tests/.helm/values.schema.json`, then run the mandatory checks above unless there is a concrete blocker.

## 17. Stability Priority

Stability is more important than micro-optimizations.
Never reduce validation coverage, weaken tests, remove assertions, or change merge semantics to make a check pass.
Prefer a small verified fix over a broad refactor.
