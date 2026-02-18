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
        {{- include "apps-utils.error" (list $ "E_RELEASE_EMPTY" "global.deploy.release resolved to empty value" "set non-empty global.deploy.release for current global.env" "docs/reference-values.md#param-global-deploy") -}}
      {{- end -}}
      {{- $_ := set $ "CurrentReleaseVersion" $currentRelease -}}

      {{- if not (hasKey $global "releases") -}}
        {{- include "apps-utils.error" (list $ "E_RELEASES_MISSING" "global.deploy.release requires global.releases map" "define global.releases.<release> with app versions" "docs/reference-values.md#param-global-releases") -}}
      {{- end -}}
      {{- if not (kindIs "map" $global.releases) -}}
        {{- include "apps-utils.error" (list $ "E_RELEASES_TYPE" "global.releases must be a map" "set global.releases as map: releaseName -> appKey -> version" "docs/reference-values.md#param-global-releases") -}}
      {{- end -}}

      {{- $releaseVersions := index $global.releases $currentRelease -}}
      {{- if not (kindIs "map" $releaseVersions) -}}
        {{- include "apps-utils.error" (list $ "E_RELEASE_NOT_FOUND" (printf "release '%s' not found in global.releases" $currentRelease) "add release key in global.releases or adjust global.deploy.release env-map" "docs/reference-values.md#param-global-releases") -}}
      {{- end -}}

      {{- $versionKey := $.CurrentApp.name -}}
      {{- if hasKey $.CurrentApp "versionKey" -}}
        {{- $versionKey = include "fl.value" (list $ $.CurrentApp $.CurrentApp.versionKey) | trim -}}
      {{- end -}}
      {{- if empty $versionKey -}}
        {{- include "apps-utils.error" (list $ "E_VERSION_KEY_EMPTY" (printf "versionKey is empty for app '%s'" $.CurrentApp.name) "set versionKey or remove it to fallback to app name" "docs/reference-values.md#param-versionkey") -}}
      {{- end -}}

      {{- $appVersion := index $releaseVersions $versionKey -}}
      {{- if $appVersion -}}
        {{- $_ := set $.CurrentApp "CurrentAppVersion" (include "fl.value" (list $ $.CurrentApp $appVersion)) -}}
        {{- if $deployEnabled -}}
          {{- $_ := set $.CurrentApp "enabled" true -}}
        {{- end -}}
      {{- end -}}
    {{- else if $deployEnabled -}}
      {{- include "apps-utils.error" (list $ "E_RELEASE_REQUIRED" "global.deploy.enabled=true requires global.deploy.release" "set global.deploy.release or disable global.deploy.enabled" "docs/reference-values.md#param-global-deploy") -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}
