{{- define "apps-check-password" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- $val := index . 2 }}
{{- $msg := index . 3 }}
{{- $password := include "fl.value" (list $ $RelatedScope $val) }}
{{- if eq $password "!!!CHANGE_ME!!!" }}
{{- fail $msg }}
{{- else }}
{{- $password }}
{{- end }}
{{- end -}}

{{- define "apps-utils.currentPath" -}}
{{- $ := index . 0 -}}
{{- if and (hasKey $ "CurrentPath") (kindIs "slice" $.CurrentPath) (gt (len $.CurrentPath) 0) -}}
{{- join "." $.CurrentPath -}}
{{- else -}}
Values
{{- end -}}
{{- end -}}

{{- define "apps-utils.error" -}}
{{- $ := index . 0 -}}
{{- $code := index . 1 -}}
{{- $message := index . 2 -}}
{{- $hint := "" -}}
{{- $docs := "" -}}
{{- $path := include "apps-utils.currentPath" (list $) | trim -}}
{{- if ge (len .) 4 -}}
{{- $hint = index . 3 -}}
{{- end -}}
{{- if ge (len .) 5 -}}
{{- $docs = index . 4 -}}
{{- end -}}
{{- if ge (len .) 6 -}}
{{- $explicitPath := index . 5 -}}
{{- if ne (toString $explicitPath) "" -}}
{{- $path = toString $explicitPath -}}
{{- end -}}
{{- end -}}
{{- $parts := list (printf "[helm-apps:%s] %s" $code $message) (printf "path=%s" $path) -}}
{{- if ne $hint "" -}}
{{- $parts = append $parts (printf "hint=%s" $hint) -}}
{{- end -}}
{{- if ne $docs "" -}}
{{- $parts = append $parts (printf "docs=%s" $docs) -}}
{{- end -}}
{{- fail (join " | " $parts) -}}
{{- end -}}

{{- define "fl.generateContainerImageQuoted" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- $imageConfig := index . 2 }}
{{- $imageName := include "fl.value" (list $ . $imageConfig.name) }}
{{- if include "fl.value" (list $ . $imageConfig.staticTag) }}
{{- $imageName }}:{{ include "fl.value" (list $ . $imageConfig.staticTag) }}
{{- else if hasKey $.CurrentApp "CurrentAppVersion" }}
{{- $imageName }}:{{ include "fl.value" (list $ . $.CurrentApp.CurrentAppVersion) }}
{{- else -}}
{{- with $.Values.werf }}
{{- index .image $imageName }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-utils.generateSpecs" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- $specs := index . 2 -}}
{{- if hasKey $specs "Required" }}
{{- range $_, $specName := $specs.Required }}
{{- $_ := include "apps-utils.requiredValue" (list $ $relativeScope $specName) }}
{{- end }}
{{- end }}
{{- with $specs.Lists }}
{{- range $_, $specName := . }}
{{- if hasPrefix "apps-" $specName }}
{{- with  include $specName (list $ $relativeScope (index $relativeScope .)) | trim }}
{{ $relativeScope.__specName__ }}: {{ print . | nindent 0 }}
{{- end }}
{{- else }}
{{- if hasKey $relativeScope $specName }}
{{- with  include "fl.value" (list $ $relativeScope (index $relativeScope .)) | trim }}
{{ $specName }}: {{ print . | trim | nindent 0 }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- with $specs.Maps }}
{{- range $_, $specName := . }}
{{- if hasPrefix "apps-" $specName }}
{{- with  include $specName (list $ $relativeScope (index $relativeScope .)) | trim }}
{{ $relativeScope.__specName__ }}: {{ print . | nindent 2 }}
{{- end }}
{{- else }}
{{- if hasKey $relativeScope $specName }}
{{- with  include "fl.value" (list $ $relativeScope (index $relativeScope .)) | trim }}
{{ $specName }}: {{ print . | nindent 2 }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- with $specs.Strings }}
{{- range $_, $specName := . }}
{{- if hasPrefix "apps-" $specName }}
{{- with  include $specName (list $ $relativeScope (index $relativeScope .)) }}
{{ $relativeScope.__specName__ }}: {{ print . | quote }}
{{- end }}
{{- else }}
{{- if hasKey $relativeScope $specName }}
{{- with  include "fl.valueQuoted" (list $ $relativeScope (index $relativeScope .)) }}
{{ $specName }}: {{ . }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- with $specs.Numbers }}
{{- range $_, $specName := . }}
{{- if hasPrefix "apps-" $specName }}
{{- with  include $specName (list $ $relativeScope (index $relativeScope .)) }}
{{ $relativeScope.__specName__ }}: {{ print . }}
{{- end }}
{{- else }}
{{- if hasKey $relativeScope $specName }}
{{- with  include "fl.value" (list $ $relativeScope (index $relativeScope .)) }}
{{ $specName }}: {{ . }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- with $specs.Bools }}
{{- range $_, $specName := . }}
{{- if hasPrefix "apps-" $specName }}
{{- $specValue := include $specName (list $ $relativeScope (index $relativeScope .)) | trim }}
{{- if  $specValue }}
{{ $relativeScope.__specName__ }}: {{ print $specValue }}
{{- end }}
{{- else }}
{{- if and (hasKey $relativeScope $specName) (ne (include "fl.value" (list $ $relativeScope (index $relativeScope .))) "") }}
{{ $specName }}: {{ include "fl.isTrue" (list $ $relativeScope (index $relativeScope .)) }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-utils.requiredValue" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- $VarName := index . 2 }}
{{- $val := include "fl.value" (list $ $RelatedScope (index $RelatedScope $VarName )) | trim }}
{{- if eq $val "" }}
{{- include "apps-utils.error" (list $ "E_REQUIRED_VALUE" (printf "required value '%s' is empty" $VarName) "set a non-empty value or disable the related block via enabled=false" "docs/reference-values.md") }}
{{- end }}
{{- $val }}
{{- end }}
{{- define "apps-utils.enterScope" }}
{{- $ := index . 0 }}
{{- $ScopeName := index . 1 }}
{{- if not (kindIs "slice" $.CurrentPath) }}
{{- $_ := set $ "CurrentPath" list }}
{{- end }}
{{- $path := append $.CurrentPath $ScopeName }}
{{- $_ := set $ "CurrentPath" $path }}
{{- end }}
{{- define "apps-utils.leaveScope" }}
{{- $_ := set . "CurrentPath" (initial .CurrentPath ) }}
{{- end }}
{{- define "apps-utils.preRenderHooks" }}
{{- $ := . }}
{{- include "apps-release.prepareApp" $ }}
{{- if hasKey $ "CurrentGroupVars" }}
{{- if hasKey $.CurrentGroupVars "_preRenderAppHook" }}
{{- $_ := include "fl.value" (list $ $.CurrentApp $.CurrentGroupVars._preRenderAppHook) }}
{{- end }}
{{- if hasKey $.CurrentGroupVars "_groupPreRenderHook" }}
{{- $_ := include "fl.value" (list $ $.CurrentApp $.CurrentGroupVars._groupPreRenderHook) }}
{{- end }}
{{- end }}
{{- if  hasKey $.CurrentApp "_preRenderHook" }}
{{- $_ := include "fl.value" (list $ $.CurrentApp $.CurrentApp._preRenderHook) }}
{{- end }}
{{- end -}}

{{- define "apps-utils.renderApps" }}
{{- $ := index . 0 }}
{{- $appScope := index . 1 }}
{{- include "apps-utils.enterScope" (list $ $appScope.__GroupVars__.name) }}
{{- include "_apps-utils.initCurrentGroupVars" (list $ $appScope $appScope.__GroupVars__.name) }}
{{- range $_appName, $_app := omit $appScope "global" "enabled" "_include" "__GroupVars__" "__AppType__" }}
{{- if hasKey . "__GroupVars__" }}
{{- include "_apps-utils.initCurrentGroupVars" (list $ . $_appName) }}
{{- if include "fl.isTrue" (list $ . $.CurrentGroupVars.enabled) }}
{{- include "apps-utils.renderApps" (list $ . $.CurrentGroupVars.type) }}
{{- end }}
{{- include "_apps-utils.initCurrentGroupVars" (list $ $appScope $appScope.__GroupVars__.name) }}
{{- else }}
{{- include "apps-utils.enterScope" (list $ $_appName) }}
{{- $type := $.CurrentGroupVars.type }}
{{- if  hasKey . "__AppType__" }}
{{- $type = .__AppType__ }}
{{- end }}
{{- if not (eq $type "__DO_NOT_RENDER__") }}
{{- $_ := set . "__AppName__" $_appName }}
{{- if hasKey . "name" }}
{{- $_ := set . "name" (include "fl.value" (list $ . .name)) }}
{{- else }}
{{- $_ := set . "name" $_appName }}
{{- end }}
{{- $_ := set $ "CurrentApp" . }}
{{- include "apps-utils.preRenderHooks" $ }}
{{- if (include "fl.isTrue" (list $ . .randomName)) }}
{{- $_ := set . "name" (printf "%s-%s" .name (randAlphaNum 7 | lower)) }}
{{- end }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- if not .__Rendered__ }}
{{- include "apps-utils.printPath" $ }}
{{- include "apps-helpers.activateContainerForDefault" $ }}
{{- include (printf "%s.render" $type) $ }}
{{- end }}
{{- end }}
{{- $_ = set . "__Rendered__" true }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end -}}

{{- define "_apps-utils.initCurrentGroupVars" }}
{{- $ := index . 0 }}
{{- $groupScope := index . 1 }}
{{- $groupName :=  index . 2 }}
{{- $_ := set $groupScope.__GroupVars__ "name" $groupName }}
{{- if kindIs "invalid" $groupScope.__GroupVars__.type }}
{{- if not (kindIs "invalid" $.CurrentGroupVars) }}
{{- $_ = set $groupScope.__GroupVars__ "type" (include "apps-utils.requiredValue" (list $ $.CurrentGroupVars "type")) }}
{{- end }}
{{- else }}
{{- $_ = set $groupScope.__GroupVars__ "type" (include "fl.value" (list $ $groupScope $groupScope.__GroupVars__.type)) }}
{{- end }}
{{- $_ = set $ "CurrentGroupVars" $groupScope.__GroupVars__ }}
{{- $_ = set $ "CurrentGroup" $groupScope }}
{{- if not (hasKey $.CurrentGroupVars "enabled") }}
{{- $_ := set $.CurrentGroupVars "enabled" true }}
{{- end }}
{{- if hasKey $groupScope.__GroupVars__ "_preRenderGroupHook" }}
{{- $_ = include "fl.value" (list $ $groupScope $groupScope.__GroupVars__._preRenderGroupHook) }}
{{- end }}
{{- end -}}

{{- define "apps-utils.init-library" }}
{{- $ := . }}
{{- include "apps-utils.includesFromFiles" $ }}
{{- $_ := include "fl.expandIncludesInValues" (list $ $.Values) }}
{{- include "apps-compat.assertNoUnexpectedLists" (list $ $.Values (list "Values")) }}
{{- include "apps-utils.findApps" $ }}
---
# Source: apps.utils:  fl.expandIncludesInValues
{{- $Library := list
"stateless"
"stateful"
"ingresses"
"cronjobs"
"jobs"
"configmaps"
"secrets"
"dex-clients"
"dex-authenticators"
"limit-range"
"kafka-strimzi"
"custom-prometheus-rules"
"grafana-dashboards"
"infra"
"pvcs"
"certificates"
"services"
"network-policies"
}}
{{- $knownTopLevel := list "global" "enabled" "_include" "werf" "helm-apps" }}
{{- range $app := $Library }}
{{- $knownTopLevel = append $knownTopLevel (printf "apps-%s" $app) }}
{{- end }}
{{- $strict := false }}
{{- with $.Values.global }}
{{- with .validation }}
{{- if include "fl.isTrue" (list $ . .strict) }}
{{- $strict = true }}
{{- end }}
{{- end }}
{{- end }}
{{- if $strict }}
{{- include "apps-compat.validateTopLevelStrict" (list $ $.Values $knownTopLevel) }}
{{- end }}
{{- range $app := $Library }}
{{- include (printf "apps-%s" $app) (list $ (index $.Values (printf "apps-%s" $app))) }}
{{- end }}
---
{{- end -}}

{{- define "apps-utils.findApps" }}
{{- $ := . }}
{{- range $groupName, $group := omit $.Values "global" "enabled" "_include" }}
{{- if kindIs "map" $group }}
{{- if hasKey $group "__GroupVars__" }}
{{- if not (kindIs "map" $group.__GroupVars__) }}
{{- $_ := set $group "__GroupVars__" dict }}
{{- end }}
{{- if not (hasKey $group.__GroupVars__ "enabled") }}
{{- $_ := set $group.__GroupVars__ "enabled" true }}
{{- end }}
{{- $_ := set $group.__GroupVars__ "name" $groupName }}
{{- if include "fl.isTrue" (list $ . $group.__GroupVars__.enabled) }}
{{- include "apps-utils.renderApps" (list $ .) }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-utils.printPath" }}
{{- printf "\n---\n# Helm Apps Library: %s" (.CurrentPath | join ".") }}
{{- end }}

{{- define "apps-utils.includesFromFiles" }}
{{- $_ := set $ "HelmAppsArgs" (dict "owner" . "current" .Values "currentName" "Values")}}
{{- include "apps-utils._includesFromFiles" (list . . .Values "Values") }}
{{- end }}

{{- define "apps-utils._includesFromFiles" }}
{{- $ := index . 0 }}
{{- $owner := index . 1 }}
{{- $current := index . 2 }}
{{- $currentName := index . 3 }}
{{- if kindIs "map" $current }}
{{- if hasKey $current "_include_from_file" }}
{{- $fn := include "apps-utils.tpl" (list $ $current._include_from_file) }}
{{- $rawContent := $.Files.Get $fn }}
{{- $includeContent := $rawContent | fromYaml }}
{{- if and (ne (trim (toString $rawContent)) "") (not $includeContent) }}
{{- $basePath := $currentName }}
{{- if not (hasPrefix "Values" $basePath) }}
{{- $basePath = printf "Values.%s" $basePath }}
{{- end }}
{{- $basePath = regexReplaceAll "^Values\\.Values(\\.|$)" $basePath "Values$1" }}
{{- include "apps-utils.error" (list $ "E_INCLUDE_FROM_FILE" (printf "_include_from_file failed to parse '%s'" $fn) "ensure file exists and contains valid YAML map" "docs/reference-values.md#param-global-includes" (printf "%s._include_from_file" $basePath)) }}
{{- end }}
{{- $currentDict := deepCopy $current}}
{{- $_ := mergeOverwrite $includeContent $currentDict }}
{{- $_ = mergeOverwrite $current $includeContent }}
{{- $_ = unset $current "_include_from_file"}}
{{- end }}
{{- if hasKey $current "_include_files" }}
{{- $newInclude := list }}
{{- range $_, $fileName := $current._include_files }}
{{- $fn := include "apps-utils.tpl" (list $ $fileName) }}
{{- $rawContent := $.Files.Get $fn }}
{{- $includeContent := $rawContent | fromYaml }}
{{- if and (ne (trim (toString $rawContent)) "") (not $includeContent) }}
{{- $basePath := $currentName }}
{{- if not (hasPrefix "Values" $basePath) }}
{{- $basePath = printf "Values.%s" $basePath }}
{{- end }}
{{- $basePath = regexReplaceAll "^Values\\.Values(\\.|$)" $basePath "Values$1" }}
{{- include "apps-utils.error" (list $ "E_INCLUDE_FILES_PARSE" (printf "_include_files failed to parse '%s'" $fn) "ensure each file exists and contains valid YAML map" "docs/reference-values.md#param-global-includes" (printf "%s._include_files" $basePath)) }}
{{- end }}
{{- $includeName := sha256sum $fileName }}
{{- $_ := set $.Values.global._includes $includeName $includeContent }}
{{- $newInclude = append $newInclude $includeName }}
{{- end }}
{{- if hasKey $current "_include" }}
{{- $newInclude = concat $newInclude $current._include }}
{{- end }}
{{- $_ := set $current "_include" $newInclude }}
{{- $_ = unset $current "_include_files"}}
{{- end }}
{{- range $k, $v :=  $current }}
{{- if kindIs "map" $v }}
{{- include "apps-utils._includesFromFiles" (list $ $current $v $k) }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}

{{- define "apps-utils.tpl" }}
{{- $ := index . 0 }}
{{- $value := index . 1 }}
{{- tpl $value  $ }}
{{- end }}
