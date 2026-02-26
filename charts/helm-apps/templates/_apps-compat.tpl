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
{{- include "apps-utils.error" (list $ "E_STRICT_UNKNOWN_KEY" (printf "unknown key '%s' in strict mode" $key) "remove the unsupported key or disable strict mode for migration period" "docs/reference-values.md#2-global" (printf "%s.%s" $scopePath $key)) }}
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
{{- include "apps-utils.error" (list $ "E_STRICT_UNKNOWN_GROUP" (printf "unknown top-level apps group '%s' in strict mode" $key) "use built-in apps-* group or define custom group with __GroupVars__.type" "docs/reference-values.md#param-custom-groups" (printf "Values.%s" $key)) }}
{{- end }}
{{- end }}
{{- end }}
{{- end -}}

{{- define "apps-compat.assertNoUnexpectedLists" -}}
{{- $ := index . 0 -}}
{{- $value := index . 1 -}}
{{- $path := index . 2 -}}
{{- $pathString := join "." $path -}}
{{- if kindIs "slice" $value -}}
  {{- $last := "" -}}
  {{- if gt (len $path) 0 -}}
    {{- $last = last $path -}}
  {{- end -}}
  {{- $isAllowedKafkaHosts := regexMatch "^Values\\.apps-kafka-strimzi\\..*\\.kafka\\.brokers\\.hosts\\.[^.]+$" $pathString -}}
  {{- $isAllowedKafkaDexGroups := regexMatch "^Values\\.apps-kafka-strimzi\\..*\\.kafka\\.ui\\.dex\\.allowedGroups\\.[^.]+$" $pathString -}}
  {{- $isAllowedGlobalInclude := regexMatch "^Values\\.global\\._includes\\..*" $pathString -}}
  {{- $isAllowedConfigFilesYAMLContent := regexMatch "^Values\\..*\\.configFilesYAML\\..*\\.content\\..*" $pathString -}}
  {{- $isAllowedEnvYAML := regexMatch "^Values\\..*\\.envYAML\\..*" $pathString -}}
  {{- $isAllowedExtraFieldsAnyLevel := regexMatch "^Values\\..*\\.extraFields(\\..*)?$" $pathString -}}
  {{- $isAllowedServiceAccountRbacRuleList := regexMatch "^Values\\.apps-service-accounts\\.[^.]+\\.(roles|clusterRoles)\\.[^.]+\\.rules\\.[^.]+\\.(apiGroups|resources|verbs|resourceNames|nonResourceURLs)$" $pathString -}}
  {{- $isAllowedContainerSharedEnvConfigMaps := regexMatch "^Values\\..*\\.containers\\.[^.]+\\.sharedEnvConfigMaps$" $pathString -}}
  {{- $isAllowedInitContainerSharedEnvConfigMaps := regexMatch "^Values\\..*\\.initContainers\\.[^.]+\\.sharedEnvConfigMaps$" $pathString -}}
  {{- $isAllowedContainerSharedEnvSecrets := regexMatch "^Values\\..*\\.containers\\.[^.]+\\.sharedEnvSecrets$" $pathString -}}
  {{- $isAllowedInitContainerSharedEnvSecrets := regexMatch "^Values\\..*\\.initContainers\\.[^.]+\\.sharedEnvSecrets$" $pathString -}}
  {{- if not (or (eq $last "_include") (eq $last "_include_files") $isAllowedGlobalInclude $isAllowedKafkaHosts $isAllowedKafkaDexGroups $isAllowedConfigFilesYAMLContent $isAllowedEnvYAML $isAllowedExtraFieldsAnyLevel $isAllowedServiceAccountRbacRuleList $isAllowedContainerSharedEnvConfigMaps $isAllowedInitContainerSharedEnvConfigMaps $isAllowedContainerSharedEnvSecrets $isAllowedInitContainerSharedEnvSecrets) -}}
    {{- include "apps-utils.error" (list $ "E_UNEXPECTED_LIST" "native YAML list is not allowed here" "for Kubernetes list fields use YAML block string ('|'); native lists are allowed only for _include/_include_files and documented exceptions" "docs/faq.md#2-почему-list-в-values-почти-везде-запрещены" $pathString) -}}
  {{- end -}}
{{- else if kindIs "map" $value -}}
  {{- range $k, $v := $value -}}
    {{- include "apps-compat.assertNoUnexpectedLists" (list $ $v (append $path $k)) -}}
  {{- end -}}
{{- end -}}
{{- end -}}
