{{- define "apps-secrets" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
    {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-secrets" "name" "apps-secrets") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
{{- end -}}
{{- end -}}

{{- define "apps-secrets.render" }}
{{- $ := . }}
{{- $_ := set $ "CurrentSecret" $.CurrentApp }}
{{- with $.CurrentApp }}
apiVersion: v1
kind: Secret
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
{{- with include "apps-compat.renderRaw" (list $ . .extraFields) | trim }}
{{- . | nindent 0 }}
{{- end }}
type: {{ include "fl.value" (list $ . .type) | default "Opaque" }}
data:
{{- if (include "fl.value" (list $ . .data) | trim) }}
{{- include "fl.value" (list $ . .data) | trim | nindent 2 }}
{{- else }}
{{- with include "fl.generateSecretEnvVars" (list $ . .envVars) | trim }}
{{- . | nindent 2 }}
{{- end }}
{{- with include "fl.generateSecretData" (list $ . .data) | trim }}
{{- . | nindent 2 }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
