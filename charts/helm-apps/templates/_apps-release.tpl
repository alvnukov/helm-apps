{{- define "apps-release.logicDisabled" -}}
{{- $ := . -}}
{{- $relativeScope := dict -}}
{{- if and (hasKey $ "CurrentApp") (kindIs "map" $.CurrentApp) -}}
  {{- $relativeScope = $.CurrentApp -}}
{{- end -}}
{{- $releaseLogicEnabled := false -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "deploy") (kindIs "map" $.Values.global.deploy) (hasKey $.Values.global.deploy "enabled") -}}
  {{- if include "fl.isTrue" (list $ $relativeScope $.Values.global.deploy.enabled) -}}
    {{- $releaseLogicEnabled = true -}}
  {{- end -}}
{{- end -}}
{{- if not $releaseLogicEnabled -}}
true
{{- end -}}
{{- end -}}

{{- define "apps-release.autoEnableAppsEnabled" -}}
{{- $ := . -}}
{{- $relativeScope := dict -}}
{{- if and (hasKey $ "CurrentApp") (kindIs "map" $.CurrentApp) -}}
  {{- $relativeScope = $.CurrentApp -}}
{{- end -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "deploy") (kindIs "map" $.Values.global.deploy) -}}
  {{- $deploy := $.Values.global.deploy -}}
  {{- if hasKey $deploy "autoEnableApps" -}}
    {{- if include "fl.isTrue" (list $ $relativeScope $deploy.autoEnableApps) -}}
true
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-release.prepareApp" -}}
{{- $ := . -}}
{{- if hasKey $ "CurrentReleaseVersion" -}}
  {{- $_ := unset $ "CurrentReleaseVersion" -}}
{{- end -}}
{{- if and (hasKey $ "CurrentApp") (kindIs "map" $.CurrentApp) (hasKey $.CurrentApp "CurrentAppVersion") -}}
  {{- $_ := unset $.CurrentApp "CurrentAppVersion" -}}
{{- end -}}
{{- $releaseLogicDisabled := eq (include "apps-release.logicDisabled" $ | trim) "true" -}}
{{- $autoEnableApps := eq (include "apps-release.autoEnableAppsEnabled" $ | trim) "true" -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) -}}
  {{- $global := $.Values.global -}}
  {{- if and (hasKey $global "deploy") (kindIs "map" $global.deploy) -}}
    {{- $deploy := $global.deploy -}}
    {{- if not $releaseLogicDisabled -}}
      {{- if hasKey $deploy "release" -}}
        {{- $currentRelease := include "fl.value" (list $ $.CurrentApp $deploy.release) | trim -}}
        {{- if empty $currentRelease -}}
          {{- include "apps-utils.error" (list $ "E_RELEASE_EMPTY" "global.deploy.release resolved to empty value" "set non-empty global.deploy.release for current global.env" "docs/reference-values.md#param-global-deploy") -}}
        {{- end -}}

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

        {{- if hasKey $releaseVersions $versionKey -}}
          {{- $resolvedAppVersion := include "fl.value" (list $ $.CurrentApp (index $releaseVersions $versionKey)) | trim -}}
          {{- if not (empty $resolvedAppVersion) -}}
          {{- $_ := set $ "CurrentReleaseVersion" $currentRelease -}}
          {{- $_ := set $.CurrentApp "CurrentAppVersion" $resolvedAppVersion -}}
          {{- if $autoEnableApps -}}
            {{- $_ := set $.CurrentApp "enabled" true -}}
          {{- end -}}
          {{- end -}}
        {{- end -}}
      {{- else -}}
        {{- include "apps-utils.error" (list $ "E_RELEASE_REQUIRED" "global.deploy.enabled=true requires global.deploy.release" "set global.deploy.release or disable global.deploy.enabled" "docs/reference-values.md#param-global-deploy") -}}
      {{- end -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}
