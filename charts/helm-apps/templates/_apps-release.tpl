{{- define "apps-release.prepareApp" -}}
{{- $ := . -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) -}}
  {{- $global := $.Values.global -}}
  {{- if and (hasKey $global "deploy") (kindIs "map" $global.deploy) -}}
    {{- $deploy := $global.deploy -}}
    {{- $deployEnabled := false -}}
    {{- if hasKey $deploy "enabled" -}}
      {{- $deployEnabled = include "fl.isTrue" (list $ $.CurrentApp $deploy.enabled) -}}
    {{- end -}}

    {{- if hasKey $deploy "release" -}}
      {{- $currentRelease := include "fl.value" (list $ $.CurrentApp $deploy.release) | trim -}}
      {{- if empty $currentRelease -}}
        {{- fail "global.deploy.release resolved to empty value" -}}
      {{- end -}}
      {{- $_ := set $ "CurrentReleaseVersion" $currentRelease -}}

      {{- if not (hasKey $global "releases") -}}
        {{- fail "global.deploy.release requires global.releases map" -}}
      {{- end -}}
      {{- if not (kindIs "map" $global.releases) -}}
        {{- fail "global.releases must be a map" -}}
      {{- end -}}

      {{- $releaseVersions := index $global.releases $currentRelease -}}
      {{- if not (kindIs "map" $releaseVersions) -}}
        {{- fail (printf "Release not found in global.releases: %s" $currentRelease) -}}
      {{- end -}}

      {{- $versionKey := $.CurrentApp.name -}}
      {{- if hasKey $.CurrentApp "versionKey" -}}
        {{- $versionKey = include "fl.value" (list $ $.CurrentApp $.CurrentApp.versionKey) | trim -}}
      {{- end -}}
      {{- if empty $versionKey -}}
        {{- fail (printf "Empty versionKey for app: %s" $.CurrentApp.name) -}}
      {{- end -}}

      {{- $appVersion := index $releaseVersions $versionKey -}}
      {{- if $appVersion -}}
        {{- $_ := set $.CurrentApp "CurrentAppVersion" (include "fl.value" (list $ $.CurrentApp $appVersion)) -}}
        {{- if $deployEnabled -}}
          {{- $_ := set $.CurrentApp "enabled" true -}}
        {{- end -}}
      {{- end -}}
    {{- else if $deployEnabled -}}
      {{- fail "global.deploy.enabled=true requires global.deploy.release" -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}
