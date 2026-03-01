{{- define "apps-k8s-manifests" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
  {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-k8s-manifests" "name" "apps-k8s-manifests") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
  {{- end -}}
{{- end -}}

{{- define "apps-k8s-manifests.resolveMapJson" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- $fieldName := index . 3 -}}
{{- $explicitPath := printf "%s.%s" (include "apps-utils.currentPath" (list $) | trim) $fieldName -}}
{{- if kindIs "invalid" $value -}}
{{- dict "wrapper" (dict) | toJson -}}
{{- else -}}
  {{- $mapValue := $value -}}
  {{- if kindIs "string" $value -}}
    {{- $raw := include "fl.value" (list $ $scope $value) | trim -}}
    {{- if eq $raw "" -}}
      {{- $mapValue = dict -}}
    {{- else -}}
      {{- $parsed := fromYaml $raw -}}
      {{- if not (kindIs "map" $parsed) -}}
        {{- include "apps-utils.error" (list $ "E_GENERIC_MAP_FIELD" (printf "'%s' must resolve to YAML map" $fieldName) "use YAML block string ('|') or map with key/value pairs" "docs/reference-values.md#param-apps-sections" $explicitPath) -}}
      {{- end -}}
      {{- $mapValue = $parsed -}}
    {{- end -}}
  {{- else if not (kindIs "map" $value) -}}
    {{- include "apps-utils.error" (list $ "E_GENERIC_MAP_FIELD" (printf "'%s' must be map or YAML string" $fieldName) "use YAML block string ('|') or map with key/value pairs" "docs/reference-values.md#param-apps-sections" $explicitPath) -}}
  {{- end -}}
  {{- $resolvedText := include "apps-k8s-manifests.renderRawResolved" (list $ $scope $mapValue) | trim -}}
  {{- $resolvedMap := dict -}}
  {{- if ne $resolvedText "" -}}
    {{- $resolvedParsed := fromYaml $resolvedText -}}
    {{- if not (kindIs "map" $resolvedParsed) -}}
      {{- include "apps-utils.error" (list $ "E_GENERIC_MAP_FIELD" (printf "'%s' must resolve to YAML map" $fieldName) "check env-map branches and rendered YAML types" "docs/reference-values.md#param-apps-sections" $explicitPath) -}}
    {{- end -}}
    {{- $resolvedMap = $resolvedParsed -}}
  {{- end -}}
  {{- if not (kindIs "map" $resolvedMap) -}}
    {{- include "apps-utils.error" (list $ "E_GENERIC_MAP_FIELD" (printf "'%s' must resolve to YAML map" $fieldName) "check env-map branches and rendered YAML types" "docs/reference-values.md#param-apps-sections" $explicitPath) -}}
  {{- end -}}
{{- dict "wrapper" $resolvedMap | toJson -}}
{{- end -}}
{{- end -}}

{{- define "apps-k8s-manifests.renderRawResolved" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- if kindIs "string" $value -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- else if or (kindIs "map" $value) (kindIs "slice" $value) -}}
{{- $resolvedWrapper := (include "apps-k8s-manifests.resolveRawJson" (list $ $scope $value) | fromJson) -}}
{{ toYaml $resolvedWrapper.wrapper }}
{{- else -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- end -}}
{{- end -}}

{{- define "apps-k8s-manifests.resolveRawJson" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- if kindIs "map" $value -}}
  {{- $currentEnv := include "fl.currentEnv" (list $) | trim -}}
  {{- $regexState := "" -}}
  {{- if ne $currentEnv "" -}}
    {{- $regexState = include "_fl.getValueRegex" (list $ $value $currentEnv) -}}
  {{- end -}}
  {{- $looksLikeEnvMap := or (hasKey $value "_default") (and (ne $currentEnv "") (hasKey $value $currentEnv)) (ne $regexState "not found") -}}
  {{- if $looksLikeEnvMap -}}
    {{- $selected := nil -}}
    {{- if and (ne $currentEnv "") (hasKey $value $currentEnv) -}}
      {{- $selected = index $value $currentEnv -}}
    {{- else if ne $regexState "not found" -}}
      {{- $selected = $._CurrentFuncResult -}}
    {{- else if hasKey $value "_default" -}}
      {{- $selected = index $value "_default" -}}
    {{- end -}}
    {{- include "apps-k8s-manifests.resolveRawJson" (list $ $scope $selected) -}}
  {{- else -}}
    {{- $result := dict -}}
    {{- range $k, $v := $value -}}
      {{- $child := include "apps-k8s-manifests.resolveRawJson" (list $ $scope $v) | fromJson -}}
      {{- $_ := set $result $k $child.wrapper -}}
    {{- end -}}
    {{- dict "wrapper" $result | toJson -}}
  {{- end -}}
{{- else if kindIs "slice" $value -}}
  {{- $result := list -}}
  {{- range $_, $v := $value -}}
    {{- $child := include "apps-k8s-manifests.resolveRawJson" (list $ $scope $v) | fromJson -}}
    {{- $result = append $result $child.wrapper -}}
  {{- end -}}
  {{- dict "wrapper" $result | toJson -}}
{{- else if kindIs "string" $value -}}
  {{- dict "wrapper" (include "fl.value" (list $ $scope $value)) | toJson -}}
{{- else if kindIs "invalid" $value -}}
  {{- dict "wrapper" "" | toJson -}}
{{- else -}}
  {{- dict "wrapper" $value | toJson -}}
{{- end -}}
{{- end -}}

{{- define "apps-k8s-manifests.emitTopField" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $fieldName := index . 2 -}}
{{- $value := index . 3 -}}
{{- if kindIs "invalid" $value -}}
{{- else if kindIs "string" $value -}}
  {{- $raw := include "fl.value" (list $ $scope $value) -}}
  {{- $parsedMap := fromYaml $raw -}}
  {{- $parsedList := fromYamlArray $raw -}}
  {{- $parsedMapHasError := and (kindIs "map" $parsedMap) (hasKey $parsedMap "Error") -}}
  {{- $parsedListHasError := and (kindIs "slice" $parsedList) (eq (len $parsedList) 1) (kindIs "string" (index $parsedList 0)) (hasPrefix "error unmarshaling JSON:" (index $parsedList 0)) -}}
  {{- if and (kindIs "map" $parsedMap) (not $parsedMapHasError) -}}
{{ $fieldName }}:
{{ toYaml $parsedMap | nindent 2 }}
  {{- else if and (kindIs "slice" $parsedList) (not $parsedListHasError) -}}
{{ $fieldName }}:
{{ toYaml $parsedList | nindent 2 }}
  {{- else -}}
{{ $fieldName }}: {{ $raw | quote }}
  {{- end -}}
{{- else if or (kindIs "map" $value) (kindIs "slice" $value) -}}
  {{- $resolved := include "apps-k8s-manifests.resolveRawJson" (list $ $scope $value) | fromJson -}}
  {{- if or (kindIs "map" $resolved.wrapper) (kindIs "slice" $resolved.wrapper) -}}
{{ $fieldName }}:
{{ toYaml $resolved.wrapper | nindent 2 }}
  {{- else if kindIs "string" $resolved.wrapper -}}
{{ $fieldName }}: {{ $resolved.wrapper | quote }}
  {{- else -}}
{{ $fieldName }}: {{ $resolved.wrapper }}
  {{- end -}}
{{- else -}}
{{ $fieldName }}: {{ $value }}
{{- end -}}
{{- end -}}

{{- define "apps-k8s-manifests.render" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- $reservedKeys := list "enabled" "name" "annotations" "labels" "metadata" "apiVersion" "kind" "spec" "data" "stringData" "binaryData" "type" "immutable" "extraFields" "_include" "_preRenderHook" "randomName" "versionKey" "CurrentReleaseVersion" "CurrentAppVersion" "__AppName__" "__Rendered__" }}
{{- $metadataMap := dict -}}
{{- if hasKey . "metadata" -}}
  {{- $metadataResolved := include "apps-k8s-manifests.resolveMapJson" (list $ . .metadata "metadata") | fromJson -}}
  {{- $metadataMap = $metadataResolved.wrapper -}}
{{- end -}}
{{- if hasKey . "name" -}}
  {{- $_ := set $metadataMap "name" (include "fl.value" (list $ . .name)) -}}
{{- end -}}
{{- if and (hasKey $metadataMap "labels") (kindIs "map" (index $metadataMap "labels")) -}}
  {{- $_ := set $metadataMap "labels" (toYaml (index $metadataMap "labels")) -}}
{{- end -}}
{{- if and (hasKey $metadataMap "annotations") (kindIs "map" (index $metadataMap "annotations")) -}}
  {{- $_ := set $metadataMap "annotations" (toYaml (index $metadataMap "annotations")) -}}
{{- end -}}
{{- $metadataResidual := omit $metadataMap "name" "labels" "annotations" -}}

{{- $extraFieldsMap := dict -}}
{{- if hasKey . "extraFields" -}}
  {{- $extraFieldsResolved := include "apps-k8s-manifests.resolveMapJson" (list $ . .extraFields "extraFields") | fromJson -}}
  {{- $extraFieldsMap = $extraFieldsResolved.wrapper -}}
  {{- range $extraKey, $_ := $extraFieldsMap -}}
    {{- if has $extraKey (list "apiVersion" "kind" "metadata" "spec" "data" "stringData" "binaryData" "type" "immutable") -}}
      {{- include "apps-utils.error" (list $ "E_GENERIC_EXTRA_FIELDS_CONFLICT" (printf "extraFields contains reserved top-level key '%s'" $extraKey) "move known fields to dedicated keys (metadata/spec/data/stringData/binaryData/type/immutable)" "docs/reference-values.md#param-apps-sections" (printf "%s.extraFields.%s" (include "apps-utils.currentPath" (list $) | trim) $extraKey)) -}}
    {{- end -}}
  {{- end -}}
{{- end -}}

{{- $implicitResidual := dict -}}
{{- range $k, $v := . -}}
  {{- if or (has $k $reservedKeys) (hasPrefix "__" $k) -}}
  {{- else -}}
    {{- $_ := set $implicitResidual $k $v -}}
  {{- end -}}
{{- end -}}
{{- $extraTopLevel := mergeOverwrite (deepCopy $implicitResidual) $extraFieldsMap }}

apiVersion: {{ include "apps-utils.requiredValue" (list $ . "apiVersion") }}
kind: {{ include "apps-utils.requiredValue" (list $ . "kind") }}
{{- include "apps-helpers.metadataGenerator" (list $ $metadataMap) }}
{{- if gt (len $metadataResidual) 0 }}
{{ toYaml $metadataResidual | nindent 2 }}
{{- end }}
{{- if hasKey . "immutable" }}
immutable: {{ include "fl.value" (list $ . .immutable) }}
{{- end }}
{{- if hasKey . "type" }}
type: {{ include "fl.valueQuoted" (list $ . .type) }}
{{- end }}
{{- if hasKey . "data" }}
{{ include "apps-k8s-manifests.emitTopField" (list $ . "data" .data) }}
{{- end }}
{{- if hasKey . "stringData" }}
{{ include "apps-k8s-manifests.emitTopField" (list $ . "stringData" .stringData) }}
{{- end }}
{{- if hasKey . "binaryData" }}
{{ include "apps-k8s-manifests.emitTopField" (list $ . "binaryData" .binaryData) }}
{{- end }}
{{- if hasKey . "spec" }}
{{ include "apps-k8s-manifests.emitTopField" (list $ . "spec" .spec) }}
{{- end }}
{{- range $k, $v := $extraTopLevel }}
{{ include "apps-k8s-manifests.emitTopField" (list $ $.CurrentApp $k $v) }}
{{- end }}
{{- end }}
{{- end }}
