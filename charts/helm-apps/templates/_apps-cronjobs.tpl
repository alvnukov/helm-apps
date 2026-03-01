{{- define "apps-cronjobs" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
  {{- if not (kindIs "invalid" $RelatedScope) }}

  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-cronjobs" "name" "apps-cronjobs") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
{{- end -}}
{{- end -}}

{{- define "apps-cronjobs.render" }}
{{- $ := . }}
{{- $_ := set $ "CurrentCronJob" $.CurrentApp }}
{{- with $.CurrentApp }}
{{- if not .containers }}
{{- include "apps-utils.error" (list $ "E_APP_CONTAINERS_REQUIRED" (printf "cronjob '%s' is enabled but containers are not configured" $.CurrentApp.name) "set containers.<name>.image or disable the cronjob (enabled=false)" "docs/reference-values.md#param-containers") }}
{{- end }}
apiVersion: {{ include "apps-api-versions.cronJob" $ }}
kind: CronJob
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
spec:
{{- $specs := dict -}}
{{- $_ = set $specs "Strings" (list "schedule" "concurrencyPolicy") -}}
{{- $_ = set $specs "Numbers" (list "failedJobsHistoryLimit" "startingDeadlineSeconds" "successfulJobsHistoryLimit") -}}
{{- $_ = set $specs "Bools" (list "suspend") -}}
  {{- with include "apps-utils.generateSpecs" (list $ . $specs) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "apps-compat.renderRaw" (list $ . .extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  jobTemplate:{{ include "apps-helpers.jobTemplate" (list $ .) | trim | nindent 4 }}

{{- include "apps-components.generateConfigMapsAndSecrets" $ -}}

{{- include "apps-components.verticalPodAutoscaler" (list $ . .verticalPodAutoscaler "CronJob") -}}

{{- end }}
{{- end }}
