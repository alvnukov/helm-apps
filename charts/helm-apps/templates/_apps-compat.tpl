{{- /*
Kubernetes version gating.

A field is emitted only when it is present in the API schema of the target
Kubernetes version. Below that version the manifest itself is invalid and the
API server, kubectl --validate and admission webhooks reject it, so the library
strips the field. At or above that version the worst case is a cluster whose
feature gate is still closed silently pruning the field, which is harmless and
resolves itself on upgrade.

Gate versions are therefore schema introduction versions, not GA versions. They
are verified against the per-version Kubernetes JSON schemas; see
docs/operations.md#kubernetes-api-compatibility.
*/ -}}

{{- define "apps-compat.kubeVersion" -}}
{{- $ := index . 0 -}}
{{- $override := "" -}}
{{- if kindIs "map" $.Values.global -}}
  {{- $compat := index $.Values.global "compat" -}}
  {{- if kindIs "map" $compat -}}
    {{- $override = index $compat "kubeVersion" | default "" | toString -}}
  {{- end -}}
{{- end -}}
{{- if $override -}}
{{- $override -}}
{{- else -}}
{{- $.Capabilities.KubeVersion.GitVersion -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.kubeAtLeast" -}}
{{- $ := index . 0 -}}
{{- $min := index . 1 -}}
{{- if semverCompare (printf ">=%s-0" $min) (include "apps-compat.kubeVersion" (list $)) -}}
true
{{- end -}}
{{- end -}}

{{- /* Drop $keys from $scope when the target cluster predates $min. */ -}}
{{- define "apps-compat.pruneBelow" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $min := index . 2 -}}
{{- $keys := index . 3 -}}
{{- if and (kindIs "map" $scope) (not (include "apps-compat.kubeAtLeast" (list $ $min))) -}}
  {{- range $_, $key := $keys -}}
    {{- $_ := unset $scope $key -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.normalizeServiceSpec" -}}
{{- $ := index . 0 -}}
{{- $service := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $service "1.20" (list "allocateLoadBalancerNodePorts" "clusterIPs" "ipFamilies" "ipFamilyPolicy")) -}}
{{- include "apps-compat.pruneBelow" (list $ $service "1.21" (list "internalTrafficPolicy" "loadBalancerClass")) -}}
{{- include "apps-compat.pruneBelow" (list $ $service "1.30" (list "trafficDistribution")) -}}
{{- end -}}

{{- define "apps-compat.normalizeStatefulSetSpec" -}}
{{- $ := index . 0 -}}
{{- $app := index . 1 -}}
{{- if kindIs "map" $app -}}
  {{- /* Not a StatefulSetSpec field at any version. */ -}}
  {{- $_ := unset $app "progressDeadlineSeconds" -}}
{{- end -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.22" (list "minReadySeconds")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.23" (list "persistentVolumeClaimRetentionPolicy")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.26" (list "ordinals")) -}}
{{- end -}}

{{- define "apps-compat.normalizePodDisruptionBudgetSpec" -}}
{{- $ := index . 0 -}}
{{- $pdb := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $pdb "1.26" (list "unhealthyPodEvictionPolicy")) -}}
{{- end -}}

{{- define "apps-compat.normalizeJobSpec" -}}
{{- $ := index . 0 -}}
{{- $app := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.21" (list "completionMode")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.25" (list "podFailurePolicy")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.28" (list "backoffLimitPerIndex" "maxFailedIndexes" "podReplacementPolicy")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.30" (list "managedBy" "successPolicy")) -}}
{{- end -}}

{{- define "apps-compat.normalizeCronJobSpec" -}}
{{- $ := index . 0 -}}
{{- $app := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.24" (list "timeZone")) -}}
{{- end -}}

{{- define "apps-compat.normalizePodSpec" -}}
{{- $ := index . 0 -}}
{{- $app := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.20" (list "setHostnameAsFQDN")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.25" (list "hostUsers")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.26" (list "schedulingGates")) -}}
{{- include "apps-compat.pruneBelow" (list $ $app "1.31" (list "resourceClaims")) -}}
{{- end -}}

{{- define "apps-compat.normalizeContainerSpec" -}}
{{- $ := index . 0 -}}
{{- $container := index . 1 -}}
{{- include "apps-compat.pruneBelow" (list $ $container "1.27" (list "resizePolicy")) -}}
{{- include "apps-compat.pruneBelow" (list $ $container "1.28" (list "restartPolicy")) -}}
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

{{- define "apps-compat.renderRawResolved" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- if kindIs "string" $value -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- else if or (kindIs "map" $value) (kindIs "slice" $value) -}}
{{- $resolvedWrapper := (include "apps-compat.resolveRawJson" (list $ $scope $value) | fromJson) -}}
{{ toYaml $resolvedWrapper.wrapper }}
{{- else -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- end -}}
{{- end -}}

{{- define "apps-compat.renderListResolved" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $fieldName := index . 2 -}}
{{- $value := index . 3 -}}
{{- $explicitPath := printf "%s.%s" (include "apps-utils.currentPath" (list $) | trim) $fieldName -}}
{{- $resolved := include "apps-compat.renderListText" (list $ $scope $value) | trim -}}
{{- if eq $resolved "" -}}
{{- else -}}
  {{- $parsedList := fromYamlArray $resolved -}}
  {{- $parsedListHasError := and (kindIs "slice" $parsedList) (eq (len $parsedList) 1) (kindIs "string" (index $parsedList 0)) (hasPrefix "error unmarshaling JSON:" (index $parsedList 0)) -}}
  {{- if or (not (kindIs "slice" $parsedList)) $parsedListHasError -}}
    {{- include "apps-utils.error" (list $ "E_LIST_FIELD" (printf "'%s' must resolve to YAML list" $fieldName) "use YAML block string ('|') with list items or a native list on allowed paths" "docs/faq.md#2-почему-list-в-values-почти-везде-запрещены" $explicitPath) -}}
  {{- end -}}
{{- $resolved -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.renderListText" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $value := index . 2 -}}
{{- if kindIs "map" $value -}}
  {{- if include "apps-compat.hasEnvValueSelection" (list $ $value) | trim -}}
    {{- $selectedWrapper := include "apps-compat.selectEnvValueJson" (list $ $value) | fromJson -}}
    {{- include "apps-compat.renderListText" (list $ $scope $selectedWrapper.wrapper) -}}
  {{- else -}}
    {{- /* Keep pre-1.8 compatibility: env-aware list fields without current env/_default collapse to empty. */ -}}
  {{- end -}}
{{- else if kindIs "slice" $value -}}
  {{- /* Native lists are data passthrough. Keep structure as-is after optional root env selection. */ -}}
{{ toYaml $value }}
{{- else -}}
{{ include "fl.value" (list $ $scope $value) }}
{{- end -}}
{{- end -}}

{{- define "apps-compat.hasEnvValueSelection" -}}
{{- $ := index . 0 -}}
{{- $value := index . 1 -}}
{{- if kindIs "map" $value -}}
  {{- $currentEnv := include "fl.currentEnv" (list $) | trim -}}
  {{- $regexState := "" -}}
  {{- if ne $currentEnv "" -}}
    {{- $regexState = include "_fl.getValueRegex" (list $ $value $currentEnv) -}}
  {{- end -}}
  {{- if or (hasKey $value "_default") (and (ne $currentEnv "") (hasKey $value $currentEnv)) (ne $regexState "not found") -}}true{{- end -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.selectEnvValueJson" -}}
{{- $ := index . 0 -}}
{{- $value := index . 1 -}}
{{- $currentEnv := include "fl.currentEnv" (list $) | trim -}}
{{- $regexState := "" -}}
{{- if ne $currentEnv "" -}}
  {{- $regexState = include "_fl.getValueRegex" (list $ $value $currentEnv) -}}
{{- end -}}
{{- if and (ne $currentEnv "") (hasKey $value $currentEnv) -}}
{{- dict "wrapper" (index $value $currentEnv) | toJson -}}
{{- else if ne $regexState "not found" -}}
{{- dict "wrapper" $._CurrentFuncResult | toJson -}}
{{- else if hasKey $value "_default" -}}
{{- dict "wrapper" (index $value "_default") | toJson -}}
{{- else -}}
{{- dict "wrapper" "" | toJson -}}
{{- end -}}
{{- end -}}

{{- define "apps-compat.strictEnabled" -}}
{{- $ := index . 0 -}}
{{- $strict := false -}}
{{- with $.Values.global -}}
  {{- with .validation -}}
    {{- if include "fl.isTrue" (list $ . .strict) -}}
      {{- $strict = true -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- if $strict }}true{{ end -}}
{{- end -}}

{{- define "apps-compat.forbidLegacyServiceAccountClusterRole" -}}
{{- $ := index . 0 -}}
{{- $forbidden := false -}}
{{- with $.Values.global -}}
  {{- with .validation -}}
    {{- if include "fl.isTrue" (list $ . .forbidLegacyServiceAccountClusterRole) -}}
      {{- $forbidden = true -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- if $forbidden }}true{{ end -}}
{{- end -}}

{{- define "apps-compat.workloadAllowedKeys" -}}
{{- $type := index . 0 -}}
{{- $allowed := list
"_include"
"__AppType__"
"enabled"
"name"
"randomName"
"alwaysRestart"
"CurrentAppVersion"
"werfWeight"
"versionKey"
"childApps"
"annotations"
"labels"
"selector"
"containers"
"initContainers"
"verticalPodAutoscaler"
"serviceAccount"
"priorityClassName"
"affinity"
"tolerations"
"nodeSelector"
"volumes"
"imagePullSecrets"
"topologySpreadConstraints"
"hostAliases"
"dnsConfig"
"overhead"
"readinessGates"
"securityContext"
"dnsPolicy"
"hostname"
"nodeName"
"preemptionPolicy"
"restartPolicy"
"runtimeClassName"
"schedulerName"
"serviceAccountName"
"subdomain"
"activeDeadlineSeconds"
"priority"
"terminationGracePeriodSeconds"
"automountServiceAccountToken"
"enableServiceLinks"
"hostIPC"
"hostNetwork"
"hostPID"
"hostUsers"
"resourceClaims"
"schedulingGates"
"setHostnameAsFQDN"
"shareProcessNamespace"
"podSpecExtra"
"extraSpec"
"_preRenderHook" -}}
{{- if eq $type "apps-stateless" -}}
  {{- $allowed = concat $allowed (list
  "reloader"
  "replicas"
  "minReadySeconds"
  "progressDeadlineSeconds"
  "revisionHistoryLimit"
  "strategy"
  "podDisruptionBudget"
  "service"
  "horizontalPodAutoscaler"
  ) -}}
{{- else if eq $type "apps-stateful" -}}
  {{- $allowed = concat $allowed (list
  "reloader"
  "replicas"
  "minReadySeconds"
  "progressDeadlineSeconds"
  "revisionHistoryLimit"
  "podDisruptionBudget"
  "service"
  "updateStrategy"
  "ordinals"
  "persistentVolumeClaimRetentionPolicy"
  "podManagementPolicy"
  "volumeClaimTemplates"
  ) -}}
{{- else if eq $type "apps-daemonsets" -}}
  {{- $allowed = concat $allowed (list
  "reloader"
  "minReadySeconds"
  "revisionHistoryLimit"
  "updateStrategy"
  "podDisruptionBudget"
  "service"
  ) -}}
{{- else if eq $type "apps-jobs" -}}
  {{- $allowed = concat $allowed (list
  "completionMode"
  "backoffLimit"
  "completions"
  "parallelism"
  "ttlSecondsAfterFinished"
  "manualSelector"
  "suspend"
  "jobTemplateExtraSpec"
  "backoffLimitPerIndex"
  "maxFailedIndexes"
  "managedBy"
  "podFailurePolicy"
  "podReplacementPolicy"
  "successPolicy"
  ) -}}
{{- else if eq $type "apps-cronjobs" -}}
  {{- $allowed = concat $allowed (list
  "schedule"
  "concurrencyPolicy"
  "timeZone"
  "successfulJobsHistoryLimit"
  "failedJobsHistoryLimit"
  "startingDeadlineSeconds"
  "completionMode"
  "backoffLimit"
  "completions"
  "parallelism"
  "ttlSecondsAfterFinished"
  "manualSelector"
  "suspend"
  "jobTemplateExtraSpec"
  "backoffLimitPerIndex"
  "maxFailedIndexes"
  "managedBy"
  "podFailurePolicy"
  "podReplacementPolicy"
  "successPolicy"
  ) -}}
{{- end -}}
{{- $allowed | toJson -}}
{{- end -}}

{{- define "apps-compat.resolveRawJson" -}}
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
    {{- $selected := "" -}}
    {{- if and (ne $currentEnv "") (hasKey $value $currentEnv) -}}
      {{- $selected = index $value $currentEnv -}}
    {{- else if ne $regexState "not found" -}}
      {{- $selected = $._CurrentFuncResult -}}
    {{- else if hasKey $value "_default" -}}
      {{- $selected = index $value "_default" -}}
    {{- end -}}
    {{- include "apps-compat.resolveRawJson" (list $ $scope $selected) -}}
  {{- else -}}
    {{- $result := dict -}}
    {{- range $k, $v := $value -}}
      {{- $child := include "apps-compat.resolveRawJson" (list $ $scope $v) | fromJson -}}
      {{- $_ := set $result $k $child.wrapper -}}
    {{- end -}}
    {{- dict "wrapper" $result | toJson -}}
  {{- end -}}
{{- else if kindIs "slice" $value -}}
  {{- /* Native lists are user data, not library DSL: no tpl/env processing inside elements. */ -}}
  {{- dict "wrapper" $value | toJson -}}
{{- else if kindIs "string" $value -}}
  {{- dict "wrapper" (include "fl.value" (list $ $scope $value)) | toJson -}}
{{- else if kindIs "invalid" $value -}}
  {{- dict "wrapper" "" | toJson -}}
{{- else -}}
  {{- dict "wrapper" $value | toJson -}}
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
{{- $state := dict "renderer" "" "path" list "group" false "childGroups" false -}}
{{- if gt (len .) 3 -}}
  {{- $state = index . 3 -}}
{{- end -}}
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
  {{- /* RBAC exceptions use actual YAML keys and the renderer of the containing app. */ -}}
  {{- $isAllowedServiceAccountRbacRuleList := false -}}
  {{- $isAllowedServiceAccountBindingSubjects := false -}}
  {{- if eq $state.renderer "apps-service-accounts" -}}
    {{- $appPath := $state.path -}}
    {{- if and (eq (len $appPath) 5) (has (index $appPath 0) (list "roles" "clusterRoles")) -}}
      {{- $isAllowedServiceAccountRbacRuleList = and (eq (index $appPath 2) "rules") (has (index $appPath 4) (list "apiGroups" "resources" "verbs" "resourceNames" "nonResourceURLs")) -}}
    {{- else if and (eq (len $appPath) 4) (has (index $appPath 0) (list "roles" "clusterRoles")) -}}
      {{- $isAllowedServiceAccountBindingSubjects = and (eq (index $appPath 2) "binding") (eq (index $appPath 3) "subjects") -}}
    {{- end -}}
  {{- end -}}
  {{- $isAllowedContainerSharedEnvConfigMaps := regexMatch "^Values\\..*\\.containers\\.[^.]+\\.sharedEnvConfigMaps$" $pathString -}}
  {{- $isAllowedInitContainerSharedEnvConfigMaps := regexMatch "^Values\\..*\\.initContainers\\.[^.]+\\.sharedEnvConfigMaps$" $pathString -}}
  {{- $isAllowedContainerSharedEnvSecrets := regexMatch "^Values\\..*\\.containers\\.[^.]+\\.sharedEnvSecrets$" $pathString -}}
  {{- $isAllowedInitContainerSharedEnvSecrets := regexMatch "^Values\\..*\\.initContainers\\.[^.]+\\.sharedEnvSecrets$" $pathString -}}
  {{- $nativeListSupportEnabled := false -}}
  {{- with $.Values.global -}}
    {{- with .validation -}}
      {{- if hasKey . "allowNativeListsInBuiltInListFields" -}}
        {{- $rawNativeListSupport := .allowNativeListsInBuiltInListFields -}}
        {{- if and (kindIs "bool" $rawNativeListSupport) $rawNativeListSupport -}}
          {{- $nativeListSupportEnabled = true -}}
        {{- else if and (kindIs "string" $rawNativeListSupport) (regexMatch "^(?i:true|1|yes|on)$" (trim $rawNativeListSupport)) -}}
          {{- $nativeListSupportEnabled = true -}}
        {{- end -}}
      {{- end -}}
    {{- end -}}
  {{- end -}}
  {{- $isBuiltinListFieldPath := regexMatch "^Values\\..*\\.(accessModes|args|command|ports|tolerations|imagePullSecrets|hostAliases|topologySpreadConstraints|clusterIPs|externalIPs|ipFamilies|loadBalancerSourceRanges|extraGroups|nodeGroups|sshPublicKeys|volumes|volumeClaimTemplates)$" $pathString -}}
  {{- $isBuiltinListFieldEnvMapPath := regexMatch "^Values\\..*\\.(accessModes|args|command|ports|tolerations|imagePullSecrets|hostAliases|topologySpreadConstraints|clusterIPs|externalIPs|ipFamilies|loadBalancerSourceRanges|extraGroups|nodeGroups|sshPublicKeys|volumes|volumeClaimTemplates)\\.[^.]+$" $pathString -}}
  {{- $isAllowedBuiltinListField := and $nativeListSupportEnabled (or $isBuiltinListFieldPath $isBuiltinListFieldEnvMapPath) -}}
  {{- if not (or (eq $last "_include") (eq $last "_include_files") $isAllowedGlobalInclude $isAllowedKafkaHosts $isAllowedKafkaDexGroups $isAllowedConfigFilesYAMLContent $isAllowedEnvYAML $isAllowedExtraFieldsAnyLevel $isAllowedServiceAccountRbacRuleList $isAllowedServiceAccountBindingSubjects $isAllowedContainerSharedEnvConfigMaps $isAllowedInitContainerSharedEnvConfigMaps $isAllowedContainerSharedEnvSecrets $isAllowedInitContainerSharedEnvSecrets $isAllowedBuiltinListField) -}}
    {{- include "apps-utils.error" (list $ "E_UNEXPECTED_LIST" "native YAML list is not allowed here" "for Kubernetes list fields use YAML block string ('|'); native lists are allowed only for _include/_include_files and documented exceptions" "docs/faq.md#2-почему-list-в-values-почти-везде-запрещены" $pathString) -}}
  {{- end -}}
{{- else if kindIs "map" $value -}}
  {{- $workloads := list "apps-stateless" "apps-stateful" "apps-daemonsets" "apps-jobs" "apps-cronjobs" -}}
  {{- $builtins := concat $workloads (include "apps-utils.childAppAllowedGroups" (list $) | fromJsonArray) (list "apps-limit-range" "apps-dex-clients" "apps-dex-authenticators" "apps-custom-prometheus-rules" "apps-grafana-dashboards" "apps-kafka-strimzi" "apps-infra") -}}
  {{- range $k, $v := $value -}}
    {{- $next := dict "renderer" $state.renderer "path" (append $state.path $k) "group" false "childGroups" false -}}
    {{- $resolveGroupType := true -}}
    {{- if eq (len $path) 1 -}}
      {{- if or (has $k $builtins) (and (kindIs "map" $v) (hasKey $v "__GroupVars__")) -}}
        {{- $_ := set $next "group" true -}}
        {{- $_ = set $next "renderer" $k -}}
      {{- end -}}
    {{- else if $state.group -}}
      {{- $_ := set $next "path" list -}}
      {{- if eq $k "__GroupVars__" -}}
        {{- $_ = set $next "renderer" "" -}}
      {{- else if kindIs "map" $v -}}
        {{- if hasKey $v "__GroupVars__" -}}
          {{- $_ = set $next "group" true -}}
        {{- else if hasKey $v "__AppType__" -}}
          {{- $_ = set $next "renderer" $v.__AppType__ -}}
        {{- end -}}
      {{- end -}}
    {{- else if $state.childGroups -}}
      {{- /* renderChildApps forces the built-in group type, ignoring supplied group vars. */ -}}
      {{- $resolveGroupType = false -}}
      {{- if has $k (include "apps-utils.childAppAllowedGroups" (list $) | fromJsonArray) -}}
        {{- $_ := set $next "group" true -}}
        {{- $_ = set $next "renderer" $k -}}
        {{- $_ = set $next "path" list -}}
      {{- end -}}
    {{- else if and (has $state.renderer $workloads) (empty $state.path) (eq $k "childApps") -}}
      {{- $_ := set $next "childGroups" true -}}
      {{- $_ = set $next "path" list -}}
    {{- end -}}
    {{- if and $resolveGroupType $next.group (kindIs "map" $v) (kindIs "map" $v.__GroupVars__) (hasKey $v.__GroupVars__ "type") -}}
      {{- $_ := set $next "renderer" (include "fl.value" (list $ $v $v.__GroupVars__.type)) -}}
    {{- end -}}
    {{- include "apps-compat.assertNoUnexpectedLists" (list $ $v (append $path $k) $next) -}}
  {{- end -}}
{{- end -}}
{{- end -}}
