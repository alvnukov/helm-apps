{{- define "imported-raw-manifest.render" -}}
{{- $ := . -}}
{{- $app := $.CurrentApp -}}
{{- include "apps-utils.printPath" $ }}
apiVersion: {{ $app.apiVersion | quote }}
kind: {{ $app.kind | quote }}
metadata:
  name: {{ $app.metadataName | quote }}
{{- with $app.metadataNamespace }}
  namespace: {{ . | quote }}
{{- end }}
{{- with $app.metadataLabelsYAML }}
  labels:
{{ . | nindent 4 }}
{{- end }}
{{- with $app.metadataAnnotationsYAML }}
  annotations:
{{ . | nindent 4 }}
{{- end }}
{{- with $app.metadataRestYAML }}
{{ . | nindent 2 }}
{{- end }}
{{- if $app.fieldsScalars }}
{{- range $k := keys $app.fieldsScalars | sortAlpha }}
{{- $v := index $app.fieldsScalars $k }}
{{- $scalarYaml := (toYaml $v | trimSuffix "\n") }}
{{ $k }}: {{ $scalarYaml }}
{{- end }}
{{- end }}
{{- if $app.fieldsYAML }}
{{- range $k := keys $app.fieldsYAML | sortAlpha }}
{{ $k }}:
{{ index $app.fieldsYAML $k | nindent 2 }}
{{- end }}
{{- end }}
{{- end -}}
