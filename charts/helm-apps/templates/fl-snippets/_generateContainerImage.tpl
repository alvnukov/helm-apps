{{- define "fl.generateContainerImageQuoted" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $imageConfig := index . 2 }}

  {{- $imageName := include "fl.value" (list $ . $imageConfig.name) }}
  {{- $werfImage := "" }}
  {{- with $.Values.werf }}
  {{- with .image }}
  {{- with (index . $imageName) }}
    {{- $werfImage = . }}
  {{- end }}
  {{- end }}
  {{- end }}
  {{- $werfReportImage := "" }}
  {{- with $.Values.global }}
  {{- with .werfReport }}
  {{- with .image }}
  {{- with (index . $imageName) }}
    {{- $werfReportImage = . }}
  {{- end }}
  {{- end }}
  {{- end }}
  {{- end }}
  {{- if include "fl.value" (list $ . $imageConfig.staticTag) }}
    {{- $imageName }}:{{ include "fl.value" (list $ . $imageConfig.staticTag) }}
  {{- else if hasKey $.CurrentApp "CurrentAppVersion" }}
    {{- $imageName }}:{{ include "fl.value" (list $ . $.CurrentApp.CurrentAppVersion) }}
  {{- else if $werfImage }}
    {{- $werfImage }}
  {{- else if $werfReportImage }}
    {{- $werfReportImage }}
  {{- else -}}
  {{- end }}
{{- end }}
