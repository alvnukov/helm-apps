{{- define "apps-compat.normalizeServiceSpec" -}}
{{- $ := index . 0 -}}
{{- $service := index . 1 -}}
{{- if and $service (kindIs "map" $service) -}}
  {{- if not (semverCompare ">=1.20-0" $.Capabilities.KubeVersion.GitVersion) -}}
    {{- $_ := unset $service "allocateLoadBalancerNodePorts" -}}
    {{- $_ := unset $service "clusterIPs" -}}
    {{- $_ := unset $service "ipFamilies" -}}
    {{- $_ := unset $service "ipFamilyPolicy" -}}
  {{- end -}}
  {{- if not (semverCompare ">=1.21-0" $.Capabilities.KubeVersion.GitVersion) -}}
    {{- $_ := unset $service "loadBalancerClass" -}}
  {{- end -}}
  {{- if not (semverCompare ">=1.22-0" $.Capabilities.KubeVersion.GitVersion) -}}
    {{- $_ := unset $service "internalTrafficPolicy" -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.normalizeStatefulSetSpec" -}}
{{- $ := index . 0 -}}
{{- $app := index . 1 -}}
{{- if and $app (kindIs "map" $app) -}}
  {{- $_ := unset $app "progressDeadlineSeconds" -}}
  {{- if not (semverCompare ">=1.23-0" $.Capabilities.KubeVersion.GitVersion) -}}
    {{- $_ := unset $app "persistentVolumeClaimRetentionPolicy" -}}
  {{- end -}}
  {{- if not (semverCompare ">=1.25-0" $.Capabilities.KubeVersion.GitVersion) -}}
    {{- $_ := unset $app "minReadySeconds" -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.renderRaw" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- if kindIs "string" $value -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- else if or (kindIs "map" $value) (kindIs "slice" $value) -}}
{{ toYaml $value }}
{{- else -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- end -}}
{{- end -}}

{{- define "apps-compat.enforceAllowedKeys" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $allowed := index . 2 -}}
{{- $scopePath := index . 3 -}}
{{- if kindIs "map" $scope -}}
{{- range $key, $_ := $scope }}
{{- if and (not (has $key $allowed)) (not (hasPrefix "__" $key)) }}
{{- fail (printf "Strict mode: unknown key '%s' at %s" $key $scopePath) }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-compat.validateTopLevelStrict" -}}
{{- $ := index . 0 -}}
{{- $values := index . 1 -}}
{{- $knownTopLevel := index . 2 -}}
{{- if kindIs "map" $values -}}
{{- range $key, $val := $values }}
{{- if has $key $knownTopLevel }}
{{- else if and (kindIs "map" $val) (hasKey $val "__GroupVars__") }}
{{- else if hasPrefix "apps-" $key }}
{{- fail (printf "Strict mode: unknown top-level apps group '%s'. Use built-in apps-* group or define custom group with __GroupVars__.type" $key) }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-compat.assertNoUnexpectedLists" -}}
{{- $ := index . 0 -}}
{{- $value := index . 1 -}}
{{- $path := index . 2 -}}
{{- if kindIs "slice" $value -}}
  {{- $last := "" -}}
  {{- if gt (len $path) 0 -}}
    {{- $last = last $path -}}
  {{- end -}}
  {{- if not (or (eq $last "_include") (eq $last "_include_files")) -}}
    {{- fail (printf "Invalid values: list value is not allowed at %s. Use YAML block string ('|') for Kubernetes list fields. Allowed native lists: _include, _include_files." (join "." $path)) -}}
  {{- end -}}
{{- else if kindIs "map" $value -}}
  {{- range $k, $v := $value -}}
    {{- include "apps-compat.assertNoUnexpectedLists" (list $ $v (append $path $k)) -}}
  {{- end -}}
{{- end -}}
{{- end -}}
