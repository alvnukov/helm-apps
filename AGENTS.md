# AGENTS.md

This file defines how AI agents should work with the `helm-apps` library in this repository.

## 1. Goal

When a user asks to deploy/update an app with this library, the agent should:

1. Pick the correct `apps-*` entity.
2. Produce valid values in library style.
3. Keep behavior compatible across environments and Kubernetes versions.
4. Run required checks before finishing.

## 2. Required Library Entry Point

Any consumer chart must initialize the library with:

```yaml
{{- include "apps-utils.init-library" $ }}
```

## 3. Supported Top-Level Sections

Use these built-in groups when possible:

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

Custom groups are allowed via:

```yaml
my-group:
  __GroupVars__:
    type: apps-stateless
```

`__GroupVars__.type` may be a string or env-map.

Custom renderers are also supported:

1. Set `__GroupVars__.type` to your custom renderer name.
2. Define template `"<type>.render"` in the consumer chart templates.
3. Library will call `include (printf "%s.render" $type) $`.

Minimal example:

```yaml
custom-services:
  __GroupVars__:
    type: custom-services
  minio:
    enabled: true
```

```yaml
{{- define "custom-services.render" -}}
{{- $ := . -}}
apiVersion: v1
kind: Service
metadata:
  name: {{ $.CurrentApp.name | quote }}
spec:
  type: ExternalName
  externalName: "example.local"
{{- end -}}
```

## 4. Values Rules (Critical)

1. Environment selection is done through `global.env`.
2. Prefer env-maps (`_default`, `prod`, regex keys) for env-specific values.
3. For most Kubernetes blocks, use YAML block strings (`|`) instead of native YAML lists/maps.
4. Native YAML lists are forbidden except allowed paths:
   - `_include`
   - `_include_files`
   - `global._includes.*`
   - `*.configFilesYAML.*.content.*`
   - `*.envYAML.*`
   - `apps-kafka-strimzi.*.kafka.brokers.hosts.*`
   - `apps-kafka-strimzi.*.kafka.ui.dex.allowedGroups.*`

If a forbidden list is used, render must fail with exact values path.

## 5. Includes and Merge

1. Reuse profiles via `global._includes` + `_include`.
2. Merge is recursive for maps.
3. `_include` chains are concatenated.
4. Local app values override included values.

## 6. Release Mode

Optional release matrix mode:

- `global.release.enabled` (default `false`)
- `global.release.current`
- `global.release.versions`
- `global.release.autoEnableApps` (default `true`)
- app-level `releaseKey` (optional, fallback to app name)

Behavior:

- resolves `CurrentAppVersion`;
- uses it as image tag when `image.staticTag` is absent;
- adds annotations `helm-apps/release` and `helm-apps/app-version`.

## 7. Network Policies

For `apps-network-policies`, select implementation via `type`:

- `kubernetes` -> `networking.k8s.io/v1` + `NetworkPolicy`
- `cilium` -> `cilium.io/v2` + `CiliumNetworkPolicy`
- `calico` -> `projectcalico.org/v3` + `NetworkPolicy`

## 8. Mandatory Checks Before Final Answer

Run (or equivalent):

```bash
werf helm lint tests/.helm --values tests/.helm/values.yaml
werf helm template contracts tests/contracts
```

If you changed compatibility behavior, also check:

```bash
werf helm template tests tests/.helm --set global.env=prod --set global._includes.apps-defaults.enabled=true --kube-version 1.29.0
werf helm template tests tests/.helm --set global.env=prod --set global._includes.apps-defaults.enabled=true --kube-version 1.20.15
```

## 9. If You Modify Library Behavior

Update all relevant artifacts:

1. Templates in `charts/helm-apps/templates/`.
2. Examples in `tests/.helm/values.yaml`.
3. Schema in `tests/.helm/values.schema.json`.
4. Contract tests in `tests/contracts/`.
5. CI checks in `.github/workflows/ci.yml`.
6. Docs (`README.md`, `docs/*`) and release notes/changelog when needed.

## 10. Stability Priority

For this repository, stability is higher priority than micro-optimizations.
Avoid risky shortcuts that reduce validation coverage or change merge semantics implicitly.
