{{- define "fl.value" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $val := index . 2 }}
  {{- $currentEnv := include "fl.currentEnv" (list $) | trim }}
  {{- $prefix := "" }}  {{- /* Optional */ -}}
  {{- $suffix := "" }}  {{- /* Optional */ -}}
  {{- if gt (len .) 3 }}
    {{- $optionalArgs := index . 3 }}
    {{- if hasKey $optionalArgs "prefix" }}
      {{- $prefix = $optionalArgs.prefix }}
    {{- end }}
    {{- if hasKey $optionalArgs "suffix" }}
      {{- $suffix = $optionalArgs.suffix }}
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
    {{- include "fl._renderValue" (list $ $relativeScope $currentEnvVal $prefix $suffix) }}
  {{- else }}
    {{- include "fl._renderValue" (list $ $relativeScope $val $prefix $suffix) }}
  {{- end }}
{{- end }}

{{- define "fl._renderValue" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $val := index . 2 }}
  {{- $prefix := index . 3 }}
  {{- $suffix := index . 4 }}

  {{- if and (not (kindIs "map" $val)) (not (kindIs "slice" $val)) }}
    {{- $valAsString := toString $val }}
    {{- if not (regexMatch "^<(nil|no value)>$" $valAsString) }}
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

{{- define "fl.currentEnv" -}}
{{- $ := index . 0 -}}
{{- $env := "" -}}
{{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) -}}
{{- with $.Values.global.env }}
{{- $env = toString . }}
{{- end }}
{{- end }}
{{- $env -}}
{{- end -}}

{{- define "fl._validateTplValue" -}}
{{- $ := index . 0 -}}
{{- $value := toString (index . 1) -}}
{{- $openCount := len (regexFindAll "\\{\\{" $value -1) -}}
{{- $closeCount := len (regexFindAll "\\}\\}" $value -1) -}}
{{- if ne $openCount $closeCount -}}
{{- include "apps-utils.error" (list $ "E_TPL_DELIMITERS" "invalid template delimiters in value" "check '{{' and '}}' pairs in this string value" "docs/faq.md#flvalue-tpl-errors") -}}
{{- end -}}
{{- if or (contains "{{{" $value) (contains "}}}" $value) -}}
{{- include "apps-utils.error" (list $ "E_TPL_BRACES" "unexpected triple braces in template value" "use Go template delimiters '{{' and '}}'; if you need literal braces, escape them" "docs/faq.md#flvalue-tpl-errors") -}}
{{- end -}}
{{- end -}}

{{- define "_fl.getValueRegex" }}
{{-     $ := index . 0 }}
{{-     $val := index . 1 }}
{{-     $currentEnv := index . 2 }}
{{-     $_ := set $ "_CurrentFuncError" "" }}
{{-     $regexList := list }}
{{-     if eq $currentEnv "" }}
{{-         print "not found" }}
{{-     else }}
{{-     range $env, $value := omit $val "_default" $currentEnv }}
{{-         $env = trimPrefix "^" $env }}
{{-         $env = trimSuffix "$" $env }}
{{-         $env = printf "^%s$" $env }}
{{-         if regexMatch $env $currentEnv }}
{{-             $_ := set $ "_CurrentFuncResult" $value }}
{{-             $regexList = append $regexList $env }}
{{-         end }}
{{-     end }}
{{-       if gt (len $regexList) 1 }}
{{-           include "apps-utils.error" (list $ "E_ENV_REGEX_AMBIGUOUS" (printf "multiple env regex keys match current global.env: %s" $regexList) "leave only one matching regex key for this value" "docs/reference-values.md#param-global-env") }}
{{-       end }}
{{-       if eq (len $regexList) 0 }}
{{-           print "not found" }}
{{-       end }}
{{-     end }}
{{- end -}}

{{- define "fl.Result" }}
{{- $ := index . 0 }}
{{- $_ := set $ "_CurrentFuncResult" (index . 1) }}
{{- $_ = set $ "_CurrentFuncError" (index . 2) }}
{{- end }}
