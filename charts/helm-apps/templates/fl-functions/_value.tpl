{{- define "fl.value" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $val := index . 2 }}
  {{- $currentEnv := include "fl.currentEnv" (list $) | trim }}
  {{- $prefix := "" }}  {{- /* Optional */ -}}
  {{- $suffix := "" }}  {{- /* Optional */ -}}
  {{- $referenceStack := list }}  {{- /* Internal */ -}}
  {{- if gt (len .) 3 }}
    {{- $optionalArgs := index . 3 }}
    {{- if hasKey $optionalArgs "prefix" }}
      {{- $prefix = $optionalArgs.prefix }}
    {{- end }}
    {{- if hasKey $optionalArgs "suffix" }}
      {{- $suffix = $optionalArgs.suffix }}
    {{- end }}
    {{- if hasKey $optionalArgs "referenceStack" }}
      {{- $referenceStack = $optionalArgs.referenceStack }}
    {{- end }}
  {{- end }}

  {{- if kindIs "map" $val }}
    {{- $currentEnvVal := "" }}
    {{- if and (ne $currentEnv "") (hasKey $val $currentEnv) }}
      {{- $currentEnvVal = index $val $currentEnv }}
    {{- else if and (ne $currentEnv "") (eq (include "_fl.getValueRegex" (list $ $val $currentEnv)) "") }}
      {{- $currentEnvVal = $._CurrentFuncResult }}
    {{- else if hasKey $val "_default" }}
      {{- $currentEnvVal = index $val "_default" }}
    {{- end }}
    {{- include "fl._renderValue" (list $ $relativeScope $currentEnvVal $prefix $suffix $referenceStack) }}
  {{- else }}
    {{- include "fl._renderValue" (list $ $relativeScope $val $prefix $suffix $referenceStack) }}
  {{- end }}
{{- end }}

{{- define "fl._renderValue" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $val := index . 2 }}
  {{- $prefix := index . 3 }}
  {{- $suffix := index . 4 }}
  {{- $referenceStack := index . 5 }}

  {{- if and (not (kindIs "map" $val)) (not (kindIs "slice" $val)) }}
    {{- $valAsString := toString $val }}
    {{- if not (regexMatch "^<(nil|no value)>$" $valAsString) }}
      {{- if contains "$fl.value{" $valAsString }}
        {{- $valAsString = include "fl._expandValueReferences" (list $ $relativeScope $valAsString $referenceStack) }}
      {{- end }}
      {{- $result := "" }}
      {{- if contains "{{" $valAsString }}
        {{- include "fl._validateTplValue" (list $ $valAsString) }}
        {{- if empty $relativeScope }}
          {{- $relativeScope = $ }}  {{- /* tpl fails if $relativeScope is empty */ -}}
        {{- end }}
        {{- $result = tpl (printf "%s{{ with $.RelativeScope }}%s{{ end }}%s" $prefix $valAsString $suffix) (merge (dict "RelativeScope" $relativeScope) $) }}
      {{- else }}
        {{- $result = printf "%s%s%s" $prefix $valAsString $suffix }}
      {{- end }}
      {{- if ne $result "" }}{{ $result }}{{ end }}
    {{- end }}
  {{- end }}
{{- end -}}

{{- define "fl._expandValueReferences" -}}
{{- $ := index . 0 -}}
{{- $relativeScope := index . 1 -}}
{{- $value := toString (index . 2) -}}
{{- $referenceStack := index . 3 -}}
{{- $escapedOpen := printf "__HELM_APPS_ESCAPED_FL_VALUE_%s__" (sha256sum $value) -}}
{{- $value = replace "$$fl.value{" $escapedOpen $value -}}
{{- $pattern := "\\$fl\\.value\\{[A-Za-z0-9_-]+(\\.[A-Za-z0-9_-]+)*\\}" -}}
{{- $references := uniq (regexFindAll $pattern $value -1) -}}
{{- $withoutValidReferences := $value -}}
{{- range $reference := $references -}}
  {{- $withoutValidReferences = replace $reference "" $withoutValidReferences -}}
{{- end -}}
{{- if contains "$fl.value{" $withoutValidReferences -}}
  {{- include "apps-utils.error" (list $ "E_VALUE_REF_SYNTAX" "invalid $fl.value reference syntax" "use $fl.value{global.path} with dot-separated [A-Za-z0-9_-] path segments" "docs/reference-values.md#param-value-references") -}}
{{- end -}}
{{- range $reference := $references -}}
  {{- $path := trimSuffix "}" (trimPrefix "$fl.value{" $reference) -}}
  {{- if has $path $referenceStack -}}
    {{- include "apps-utils.error" (list $ "E_VALUE_REF_CYCLE" (printf "cyclic $fl.value reference detected: %s" (append $referenceStack $path)) "remove the reference cycle" "docs/reference-values.md#param-value-references") -}}
  {{- end -}}
  {{- $current := $.Values -}}
  {{- $found := true -}}
  {{- range $segment := splitList "." $path -}}
    {{- if and $found (kindIs "map" $current) (hasKey $current $segment) -}}
      {{- $current = index $current $segment -}}
    {{- else -}}
      {{- $found = false -}}
    {{- end -}}
  {{- end -}}
  {{- if not $found -}}
    {{- include "apps-utils.error" (list $ "E_VALUE_REF_NOT_FOUND" (printf "$fl.value reference path not found: %s" $path) "define the path under .Values or fix the reference" "docs/reference-values.md#param-value-references") -}}
  {{- end -}}
  {{- $resolved := include "fl.value" (list $ $relativeScope $current (dict "referenceStack" (append $referenceStack $path))) -}}
  {{- $value = replace $reference $resolved $value -}}
{{- end -}}
{{- replace $escapedOpen "$fl.value{" $value -}}
{{- end -}}

{{- define "fl.currentEnv" -}}
{{- $ := index . 0 -}}
{{- $env := "" -}}
{{- if and (hasKey $.Values "werf") (kindIs "map" $.Values.werf) -}}
{{- with $.Values.werf.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- if and (eq $env "") (hasKey $.Values "global") (kindIs "map" $.Values.global) -}}
{{- with $.Values.global.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- if and (eq $env "") (hasKey $ "werf") (kindIs "map" $.werf) -}}
{{- with $.werf.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- if and (eq $env "") (hasKey $ "global") (kindIs "map" $.global) -}}
{{- with $.global.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- if and (eq $env "") (hasKey $ "env") -}}
{{- with $.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- if eq (trim $env) "" -}}
{{- include "apps-utils.error" (list $ "E_ENV_REQUIRED" "environment is not set for env-aware values rendering" "set werf.env (werf --env <env>) or global.env (helm --set global.env=<env>) at render/deploy stage" "docs/reference-values.md#param-global-env") -}}
{{- end }}
{{- $env -}}
{{- end -}}

{{- define "fl._validateTplValue" -}}
{{- $ := index . 0 -}}
{{- $value := toString (index . 1) -}}
{{- if include "fl.tplDelimitersValidationEnabled" (list $) -}}
{{- $openCount := len (regexFindAll "\\{\\{" $value -1) -}}
{{- $closeCount := len (regexFindAll "\\}\\}" $value -1) -}}
{{- if ne $openCount $closeCount -}}
{{- include "apps-utils.error" (list $ "E_TPL_DELIMITERS" "invalid template delimiters in value" "check '{{' and '}}' pairs in this string value" "docs/faq.md#flvalue-tpl-errors") -}}
{{- end -}}
{{- if or (contains "{{{" $value) (contains "}}}" $value) -}}
{{- include "apps-utils.error" (list $ "E_TPL_BRACES" "unexpected triple braces in template value" "use Go template delimiters '{{' and '}}'; if you need literal braces, escape them" "docs/faq.md#flvalue-tpl-errors") -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "fl.tplDelimitersValidationEnabled" -}}
{{- $ := index . 0 -}}
{{- $enabled := false -}}
{{- with $.Values.global -}}
  {{- with .validation -}}
    {{- if hasKey . "validateTplDelimiters" -}}
      {{- $raw := .validateTplDelimiters -}}
      {{- if and (kindIs "bool" $raw) $raw -}}
        {{- $enabled = true -}}
      {{- else if and (kindIs "string" $raw) (regexMatch "^(?i:true|1|yes|on)$" (trim $raw)) -}}
        {{- $enabled = true -}}
      {{- end -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- if $enabled }}true{{ end -}}
{{- end -}}

{{- define "_fl.getValueRegex" }}
{{- $ := index . 0 }}
{{- $val := index . 1 }}
{{- $currentEnv := index . 2 }}
{{- $_ := set $ "_CurrentFuncError" "" }}
{{- $regexList := list }}
{{- if eq $currentEnv "" }}
{{- print "not found" }}
{{- else }}
{{- range $env, $value := omit $val "_default" $currentEnv }}
{{- $env = trimPrefix "^" $env }}
{{- $env = trimSuffix "$" $env }}
{{- $env = printf "^%s$" $env }}
{{- if regexMatch $env $currentEnv }}
{{- $_ := set $ "_CurrentFuncResult" $value }}
{{- $regexList = append $regexList $env }}
{{- end }}
{{- end }}
{{- if gt (len $regexList) 1 }}
{{- include "apps-utils.error" (list $ "E_ENV_REGEX_AMBIGUOUS" (printf "multiple env regex keys match current global.env: %s" $regexList) "leave only one matching regex key for this value" "docs/reference-values.md#param-global-env") }}
{{- end }}
{{- if eq (len $regexList) 0 }}
{{- print "not found" }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "fl.Result" }}
{{- $ := index . 0 }}
{{- $_ := set $ "_CurrentFuncResult" (index . 1) }}
{{- $_ = set $ "_CurrentFuncError" (index . 2) }}
{{- end }}
