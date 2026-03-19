{{- define "fl.generateContainerImageQuoted" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $imageConfig := index . 2 }}

  {{- $imageName := include "fl.value" (list $ . $imageConfig.name) }}
  {{- $imageRepository := include "fl.value" (list $ . $imageConfig.repository) }}
  {{- $taggedImageName := $imageName }}
  {{- if $imageRepository }}
    {{- $taggedImageName = printf "%s/%s" (trimSuffix "/" $imageRepository) (trimPrefix "/" $imageName) }}
  {{- end }}
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
  {{- $releaseLogicDisabled := eq (include "apps-release.logicDisabled" $ | trim) "true" }}
  {{- if include "fl.value" (list $ . $imageConfig.staticTag) }}
    {{- $taggedImageName }}:{{ include "fl.value" (list $ . $imageConfig.staticTag) }}
  {{- else if and (not $releaseLogicDisabled) (hasKey $.CurrentApp "CurrentAppVersion") }}
    {{- $taggedImageName }}:{{ include "fl.value" (list $ . $.CurrentApp.CurrentAppVersion) }}
  {{- else if $werfImage }}
    {{- $werfImage }}
  {{- else if $werfReportImage }}
    {{- $werfReportImage }}
  {{- else -}}
  {{- end }}
{{- end }}
