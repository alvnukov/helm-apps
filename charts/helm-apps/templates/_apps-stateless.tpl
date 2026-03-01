{{- define "apps-stateless" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
    {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-stateless" "name" "apps-stateless") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
{{- end -}}
{{- end -}}

{{- define "apps-stateless.render" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- if kindIs "invalid" .containers }}
{{- include "apps-utils.error" (list $ "E_APP_CONTAINERS_REQUIRED" (printf "app '%s' is enabled but containers are not configured" $.CurrentApp.name) "set containers.<name>.image or disable the app (enabled=false)" "docs/reference-values.md#param-containers") }}
{{- end }}
{{- /* Defaults values */ -}}
{{- if .service }}
{{- if include "fl.isTrue" (list $ . .service.enabled) }}
{{- if not .service.name }}
{{- $_ := set .service "name" .name }}
{{- end }}
{{- end }}
{{- end }}
{{- /* Defaults values end */ -}}
{{- $serviceAccount := include "apps-system.serviceAccount" $ -}}
apiVersion: apps/v1
kind: Deployment
{{- $_ := set . "__annotations__" dict -}}
{{- if .reloader }}
{{- $_ := set .__annotations__ "pod-reloader.deckhouse.io/auto" "true" }}
{{- else }}
{{- $_ := set . "__annotations__" (include "apps-components.generate-config-checksum" (list $ .) | fromYaml) }}
{{- end }}
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
spec:
{{- $specs := dict -}}
{{- $_ = set $specs "Numbers" (list "minReadySeconds" "progressDeadlineSeconds" "revisionHistoryLimit" "replicas") -}}
{{- $_ = set $specs "Maps" (list "strategy" "apps-helpers.podTemplate" "apps-specs.selector") -}}
  {{- with include "apps-utils.generateSpecs" (list $ . $specs) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "apps-compat.renderRaw" (list $ . .extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
{{- $_ = unset . "__annotations__" }}
{{- include "apps-components.generateConfigMapsAndSecrets" $ -}}
{{- include "apps-components.service" (list $ . .service) -}}
{{- include "apps-components.podDisruptionBudget" (list $ . .podDisruptionBudget) -}}
{{- include "apps-components.verticalPodAutoscaler" (list $ . .verticalPodAutoscaler "Deployment") }}
{{- include "apps-components.horizontalPodAutoscaler" (list $ . "Deployment") -}}
{{- include "apps-deckhouse.metrics" $ -}}
{{ $serviceAccount -}}

{{- end }}
{{- end }}
