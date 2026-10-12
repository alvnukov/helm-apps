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

{{- /* Validate only extended entry fields; preserve the legacy scalar/env path. */ -}}
{{- define "apps-release.validateValue" -}}
{{- $ := index . 0 -}}
{{- $value := index . 1 -}}
{{- $path := index . 2 -}}
{{- $stringsOnly := index . 3 -}}
{{- $leaves := list $value -}}
{{- if kindIs "map" $value -}}
  {{- $leaves = values $value -}}
{{- end -}}
{{- range $leaf := $leaves -}}
  {{- if or (kindIs "map" $leaf) (kindIs "slice" $leaf) (and $stringsOnly (not (kindIs "string" $leaf)) (ne (kindOf $leaf) "invalid")) -}}
    {{- include "apps-utils.error" (list $ "E_RELEASE_VALUE_TYPE" "invalid extended release value type" "use a scalar or an env-map with scalar leaves; image fields require strings or null" "docs/reference-values.md#param-global-releases" $path) -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-release.resolveEntry" -}}
{{- $ := index . 0 -}}
{{- $relativeScope := index . 1 -}}
{{- $entry := index . 2 -}}
{{- $path := index . 3 -}}
{{- $version := $entry -}}
{{- $image := dict -}}
{{- if and (kindIs "map" $entry) (hasKey $entry "version") -}}
  {{- range $key, $_ := $entry -}}
    {{- if not (has $key (list "version" "image")) -}}
      {{- include "apps-utils.error" (list $ "E_RELEASE_ENTRY_KEY" (printf "unknown extended release field '%s'" $key) "use version and optional image; place env selectors inside version" "docs/reference-values.md#param-global-releases" $path) -}}
    {{- end -}}
  {{- end -}}
  {{- $version = $entry.version -}}
  {{- include "apps-release.validateValue" (list $ $version (printf "%s.version" $path) false) -}}
  {{- if hasKey $entry "image" -}}
    {{- if not (kindIs "map" $entry.image) -}}
      {{- include "apps-utils.error" (list $ "E_RELEASE_IMAGE_TYPE" "extended release image must be a map" "set optional image.repository and/or image.name" "docs/reference-values.md#param-global-releases" (printf "%s.image" $path)) -}}
    {{- end -}}
    {{- $image = $entry.image -}}
    {{- range $key, $value := $image -}}
      {{- if not (has $key (list "repository" "name")) -}}
        {{- include "apps-utils.error" (list $ "E_RELEASE_IMAGE_KEY" (printf "unknown release image field '%s'" $key) "use repository and/or name; the tag belongs in version" "docs/reference-values.md#param-global-releases" (printf "%s.image" $path)) -}}
      {{- end -}}
      {{- include "apps-release.validateValue" (list $ $value (printf "%s.image.%s" $path $key) true) -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- $resolvedVersion := include "fl.value" (list $ $relativeScope $version) | trim -}}
{{- dict "version" $resolvedVersion "image" $image | toJson -}}
{{- end -}}

{{- /* Called only in the existing non-init, non-static release image branch. */ -}}
{{- define "apps-release.imageName" -}}
{{- $ := index . 0 -}}
{{- $relativeScope := index . 1 -}}
{{- $name := index . 2 -}}
{{- $repository := index . 3 -}}
{{- $image := $.CurrentReleaseImage -}}
{{- if hasKey $image "name" -}}
  {{- $name = include "fl.value" (list $ $relativeScope $image.name) -}}
  {{- if eq (trim $name) "" -}}
    {{- include "apps-utils.error" (list $ "E_RELEASE_IMAGE_EMPTY" "release image.name resolved to empty value" "set a non-empty name for this environment or omit the override" "docs/reference-values.md#param-global-releases") -}}
  {{- end -}}
{{- end -}}
{{- if hasKey $image "repository" -}}
  {{- $repository = include "fl.value" (list $ $relativeScope $image.repository) -}}
{{- end -}}
{{- if $repository -}}
  {{- printf "%s/%s" (trimSuffix "/" $repository) (trimPrefix "/" $name) -}}
{{- else -}}
  {{- $name -}}
{{- end -}}
{{- end -}}

{{- define "apps-release.prepareApp" -}}
{{- $ := . -}}
{{- if hasKey $ "CurrentReleaseImage" -}}
  {{- $_ := unset $ "CurrentReleaseImage" -}}
{{- end -}}
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
          {{- $entry := include "apps-release.resolveEntry" (list $ $.CurrentApp (index $releaseVersions $versionKey) (printf "global.releases.%s.%s" $currentRelease $versionKey)) | fromJson -}}
          {{- $resolvedAppVersion := $entry.version -}}
          {{- if not (empty $resolvedAppVersion) -}}
          {{- $_ := set $ "CurrentReleaseVersion" $currentRelease -}}
          {{- $_ := set $.CurrentApp "CurrentAppVersion" $resolvedAppVersion -}}
          {{- if not (empty $entry.image) -}}
            {{- $_ := set $ "CurrentReleaseImage" $entry.image -}}
          {{- end -}}
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
