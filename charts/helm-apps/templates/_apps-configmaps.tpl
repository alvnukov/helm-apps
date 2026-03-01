{{- define "apps-configmaps" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
    {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-configmaps" "name" "apps-configmaps") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
    {{- end -}}
{{- end -}}

{{- define "apps-configmaps.render" }}
{{- $ := . }}
{{- $_ := set $ "CurrentConfigMap" $.CurrentApp }}
{{- with $.CurrentApp }}
apiVersion: v1
kind: ConfigMap
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
{{- with include "apps-compat.renderRaw" (list $ . .extraFields) | trim }}
{{- . | nindent 0 }}
{{- end }}
{{- $data :=  "" }}
{{- with include "apps.generateConfigMapEnvVars" (list $ . .envVars) | trim }}
{{- $data = printf "%s\n%s" $data . | trim }}
{{- end }}
{{- if kindIs "map" .data }}
{{- with include "apps.generateConfigMapData" (list $ . .data) | trim }}
{{- $data = printf "%s\n%s" $data . | trim }}
{{- end }}
{{- else }}
{{- with include "fl.value" (list $ . .data) | trim }}
{{- $data = printf "%s\n%s" $data . | trim }}
{{- end }}
{{- end }}
{{- with $data }}
data:
{{- . | nindent 2 }}
{{- end }}
{{- with include "fl.value" (list $ . .binaryData) | trim }}
binaryData:
{{- . | nindent 2 }}
{{- end }}
{{- end }}
{{- end }}
