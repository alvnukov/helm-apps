{{- define "apps-jobs" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
    {{- if not (kindIs "invalid" $RelatedScope) }}

  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-jobs" "name" "apps-jobs") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
{{- end -}}
{{- end -}}

{{- define "apps-jobs.render" }}
{{- $ := . }}
{{- $_ := set $ "CurrentJob" $.CurrentApp }}
{{- with $.CurrentApp }}

{{- if not .containers }}
{{- include "apps-utils.error" (list $ "E_APP_CONTAINERS_REQUIRED" (printf "job '%s' is enabled but containers are not configured" $.CurrentApp.name) "set containers.<name>.image or disable the job (enabled=false)" "docs/reference-values.md#param-containers") }}
{{- end }}
{{- if and (kindIs "invalid" .jobTemplateExtraSpec) (not (kindIs "invalid" .extraSpec)) }}
{{- $_ := set . "jobTemplateExtraSpec" .extraSpec }}
{{- end }}
apiVersion: batch/v1
kind: Job
{{- include "apps-helpers.metadataGenerator" (list $ .) -}}

{{- include "apps-helpers.jobTemplate" (list $ .) -}}

{{- include "apps-components.generateConfigMapsAndSecrets" $ -}}

{{- include "apps-components.verticalPodAutoscaler" (list $ . .verticalPodAutoscaler "Job") -}}

{{- end }}
{{- end }}
