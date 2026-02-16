{{- define "apps-release.prepareApp" -}}
{{- $ := . -}}
{{- $releaseCfg := dict -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "release") (kindIs "map" $.Values.global.release) -}}
  {{- $releaseCfg = $.Values.global.release -}}
{{- end -}}
{{- if kindIs "map" $releaseCfg -}}
  {{- if and (hasKey $releaseCfg "enabled") (include "fl.isTrue" (list $ $.CurrentApp $releaseCfg.enabled)) -}}
    {{- $currentRelease := include "fl.value" (list $ $.CurrentApp $releaseCfg.current) -}}
    {{- if empty $currentRelease -}}
      {{- fail "global.release.enabled=true requires global.release.current" -}}
    {{- end -}}
    {{- $_ := set $ "CurrentReleaseVersion" $currentRelease -}}

    {{- $versions := $releaseCfg.versions -}}
    {{- if not (kindIs "map" $versions) -}}
      {{- fail "global.release.enabled=true requires global.release.versions map" -}}
    {{- end -}}

    {{- $releaseVersions := index $versions $currentRelease -}}
    {{- if not $releaseVersions -}}
      {{- fail (printf "Release not found in global.release.versions: %s" $currentRelease) -}}
    {{- end -}}

    {{- $releaseKey := $.CurrentApp.name -}}
    {{- if hasKey $.CurrentApp "releaseKey" -}}
      {{- $releaseKey = include "fl.value" (list $ $.CurrentApp $.CurrentApp.releaseKey) -}}
    {{- end -}}

    {{- $appVersion := index $releaseVersions $releaseKey -}}
    {{- if $appVersion -}}
      {{- $_ := set $.CurrentApp "CurrentAppVersion" (include "fl.value" (list $ $.CurrentApp $appVersion)) -}}
      {{- $autoEnable := true -}}
      {{- if hasKey $releaseCfg "autoEnableApps" -}}
        {{- $autoEnable = include "fl.isTrue" (list $ $.CurrentApp $releaseCfg.autoEnableApps) -}}
      {{- end -}}
      {{- if $autoEnable -}}
        {{- $_ := set $.CurrentApp "enabled" true -}}
      {{- end -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}
