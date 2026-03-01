{{- define "fl.generateLabels" }}
  {{- $ := index . 0 -}}
  {{- $relativeScope := index . 1 -}}
  {{- $appName := index . 2 -}}
app: {{ $appName | quote }}
chart: {{ $.Chart.Name | trunc 63 | quote }}
{{- with $.Values.werf }}
repo: {{ regexSplit "/" .repo -1 | rest | join "-" | trunc 63 | quote }}
{{- end }}
{{- $addEnvLabel := false }}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "labels") (kindIs "map" $.Values.global.labels) (hasKey $.Values.global.labels "addEnv") }}
  {{- if include "fl.isTrue" (list $ $relativeScope $.Values.global.labels.addEnv) -}}
    {{- $addEnvLabel = true -}}
  {{- end -}}
{{- end }}
{{- if $addEnvLabel }}
app.kubernetes.io/environment: {{ include "fl.currentEnv" (list $) | quote }}
{{- end }}
{{- end }}
