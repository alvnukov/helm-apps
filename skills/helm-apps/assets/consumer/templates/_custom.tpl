{{- define "demo-configmaps.render" -}}
{{- $app := $.CurrentApp -}}
apiVersion: v1
kind: ConfigMap
{{ include "apps-helpers.metadataGenerator" (list $ $app) }}
data:
  endpoint: {{ include "apps-utils.requiredValue" (list $ $app "endpoint") | quote }}
{{ if include "fl.isTrue" (list $ $app $app.emitDetails) }}
  details: "enabled"
{{ end }}
{{- end -}}
