{{- define "apps-service-accounts" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
  {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-service-accounts" "name" "apps-service-accounts") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
  {{- end -}}
{{- end -}}

{{- define "apps-service-accounts._namespace" -}}
{{- $ := index . 0 -}}
{{- $sa := index . 1 -}}
{{- $ns := include "fl.value" (list $ $sa $sa.namespace) | trim -}}
{{- if eq $ns "" -}}
{{- $ns = $.Release.Namespace | trim -}}
{{- end -}}
{{- $ns -}}
{{- end -}}

{{- define "apps-service-accounts._metadataNamespaced" -}}
{{- $ := index . 0 -}}
{{- $scope := index . 1 -}}
{{- $sa := index . 2 -}}
{{- include "apps-helpers.metadataGenerator" (list $ $scope) }}
{{- $ns := include "fl.value" (list $ $scope $scope.namespace) | trim -}}
{{- if eq $ns "" -}}
  {{- $ns = include "fl.value" (list $ $sa $sa.namespace) | trim -}}
{{- end -}}
{{- with $ns }}
  namespace: {{ . | quote }}
{{- end }}
{{- end -}}

{{- define "apps-service-accounts._rbacObjectName" -}}
{{- $ := index . 0 -}}
{{- $sa := index . 1 -}}
{{- $rbacObj := index . 2 -}}
{{- $rbacKey := index . 3 -}}
{{- $saName := $.CurrentApp.name -}}
{{- if hasKey $rbacObj "name" -}}
{{ include "fl.value" (list $ $rbacObj $rbacObj.name) }}
{{- else -}}
{{ printf "%s-%s" $saName $rbacKey }}
{{- end -}}
{{- end -}}

{{- define "apps-service-accounts._renderRuleItem" -}}
{{- $ := index . 0 -}}
{{- $rule := index . 1 -}}
{{- $ruleName := index . 2 -}}
{{- if not (kindIs "map" $rule) -}}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_RULE_INVALID" (printf "rule '%s' must be a map" $ruleName) "define rules as map: rules.<ruleName>.<field>" "docs/reference-values.md#param-apps-sections") }}
{{- end -}}
{{- if or (not (hasKey $rule "enabled")) (include "fl.isTrue" (list $ $rule $rule.enabled)) -}}
{{- if and (not (hasKey $rule "resources")) (not (hasKey $rule "nonResourceURLs")) (not (hasKey $rule "extraFields")) -}}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_RULE_EMPTY" (printf "rule '%s' has no RBAC fields" $ruleName) "set resources/nonResourceURLs (and usually verbs) or disable the rule" "docs/reference-values.md#param-apps-sections") }}
{{- end -}}
{{- if not (hasKey $rule "verbs") -}}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_RULE_VERBS_REQUIRED" (printf "rule '%s' is missing verbs" $ruleName) "set rules.<ruleName>.verbs as native list" "docs/reference-values.md#param-apps-sections") }}
{{- end -}}
-{{- with include "apps-compat.renderRaw" (list $ $rule $rule.apiGroups) | trim }}
 apiGroups:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ $rule $rule.resources) | trim }}
 resources:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ $rule $rule.verbs) | trim }}
 verbs:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ $rule $rule.resourceNames) | trim }}
 resourceNames:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ $rule $rule.nonResourceURLs) | trim }}
 nonResourceURLs:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ $rule $rule.extraFields) | trim }}
{{- . | nindent 1 }}
{{- end }}
{{- end -}}
{{- end -}}

{{- define "apps-service-accounts._renderRulesList" -}}
{{- $ := index . 0 -}}
{{- $rbacObj := index . 1 -}}
{{- $rulesMap := $rbacObj.rules -}}
{{- if kindIs "invalid" $rulesMap -}}
{{- else if not (kindIs "map" $rulesMap) -}}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_RULES_INVALID" "rules must be a map with named rules" "use rules.<ruleName>.<field>; native YAML lists are allowed only in rule leaf fields" "docs/reference-values.md#param-apps-sections") }}
{{- else -}}
rules:
{{- $ruleKeys := keys $rulesMap | sortAlpha }}
{{- range $_, $ruleName := $ruleKeys }}
  {{- $rule := index $rulesMap $ruleName }}
{{ include "apps-service-accounts._renderRuleItem" (list $ $rule $ruleName) | nindent 0 }}
{{- end }}
{{- end -}}
{{- end -}}

{{- define "apps-service-accounts._renderRoleAndBinding" -}}
{{- $ := index . 0 -}}
{{- $sa := index . 1 -}}
{{- $roleKey := index . 2 -}}
{{- $role := index . 3 -}}
{{- $kind := index . 4 -}}
{{- $bindingKind := index . 5 -}}
{{- $namespaced := index . 6 -}}
{{- if and (kindIs "map" $role) (or (not (hasKey $role "enabled")) (include "fl.isTrue" (list $ $role $role.enabled))) -}}
{{- $rbacName := include "apps-service-accounts._rbacObjectName" (list $ $sa $role $roleKey) | trim -}}
{{- $binding := dict -}}
{{- if and (hasKey $role "binding") (kindIs "map" $role.binding) -}}
  {{- $binding = deepCopy $role.binding -}}
{{- else if hasKey $role "binding" -}}
  {{- include "apps-utils.error" (list $ "E_SA_RBAC_BINDING_INVALID" (printf "%s.%s.binding must be a map" (lower $kind) $roleKey) "use binding.<field> under role/clusterRole" "docs/reference-values.md#param-apps-sections") -}}
{{- end -}}
{{- $bindingName := $rbacName -}}
{{- if hasKey $binding "name" -}}
  {{- $bindingName = include "fl.value" (list $ $binding $binding.name) | trim | default $rbacName -}}
{{- end -}}
{{- $subjectNamespace := include "apps-service-accounts._namespace" (list $ $sa) | trim -}}
{{- $roleScope := deepCopy $role -}}
{{- $_ := set $roleScope "name" $rbacName -}}
{{- $bindingScope := deepCopy $binding -}}
{{- $_ := set $bindingScope "name" $bindingName -}}

---
apiVersion: rbac.authorization.k8s.io/v1
kind: {{ $kind }}
{{- if $namespaced }}
{{- include "apps-service-accounts._metadataNamespaced" (list $ $roleScope $sa) }}
{{- else }}
{{- include "apps-helpers.metadataGenerator" (list $ $roleScope) }}
{{- end }}
{{ include "apps-service-accounts._renderRulesList" (list $ $role) }}
{{- with include "apps-compat.renderRaw" (list $ $role $role.extraFields) | trim }}
{{- . | nindent 0 }}
{{- end }}

---
apiVersion: rbac.authorization.k8s.io/v1
kind: {{ $bindingKind }}
{{- if $namespaced }}
{{- include "apps-service-accounts._metadataNamespaced" (list $ $bindingScope $sa) }}
{{- else }}
{{- include "apps-helpers.metadataGenerator" (list $ $bindingScope) }}
{{- end }}
subjects:
{{- if hasKey $binding "subjects" }}
{{- with include "apps-compat.renderRaw" (list $ $binding $binding.subjects) | trim }}
{{- . | nindent 2 }}
{{- end }}
{{- else }}
  - kind: ServiceAccount
    name: {{ $.CurrentApp.name }}
    namespace: {{ $subjectNamespace }}
{{- end }}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: {{ $kind }}
  name: {{ $rbacName }}
{{- with include "apps-compat.renderRaw" (list $ $binding $binding.extraFields) | trim }}
{{- . | nindent 0 }}
{{- end }}
{{- end -}}
{{- end -}}

{{- define "apps-service-accounts.render" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- if and (hasKey . "roles") (not (kindIs "map" .roles)) }}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_ROLES_INVALID" "roles must be a map" "use roles.<roleName>.rules.<ruleName>.<field>" "docs/reference-values.md#param-apps-sections") }}
{{- end }}
{{- if and (hasKey . "clusterRoles") (not (kindIs "map" .clusterRoles)) }}
{{- include "apps-utils.error" (list $ "E_SA_RBAC_CLUSTER_ROLES_INVALID" "clusterRoles must be a map" "use clusterRoles.<roleName>.rules.<ruleName>.<field>" "docs/reference-values.md#param-apps-sections") }}
{{- end }}
apiVersion: {{ include "fl.value" (list $ . .apiVersion) | default "v1" }}
kind: ServiceAccount
{{- include "apps-service-accounts._metadataNamespaced" (list $ . .) }}
{{- if and (hasKey . "automountServiceAccountToken") (ne (include "fl.value" (list $ . .automountServiceAccountToken)) "") }}
automountServiceAccountToken: {{ include "fl.value" (list $ . .automountServiceAccountToken) }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ . .imagePullSecrets) | trim }}
imagePullSecrets:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ . .secrets) | trim }}
secrets:
{{- . | nindent 2 }}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ . .extraFields) | trim }}
{{- . | nindent 0 }}
{{- end }}
{{- if kindIs "map" .roles }}
{{- $roleKeys := keys .roles | sortAlpha }}
{{- range $_, $roleKey := $roleKeys }}
{{- $role := index $.CurrentApp.roles $roleKey }}
{{ include "apps-service-accounts._renderRoleAndBinding" (list $ $.CurrentApp $roleKey $role "Role" "RoleBinding" true) }}
{{- end }}
{{- end }}
{{- if kindIs "map" .clusterRoles }}
{{- $clusterRoleKeys := keys .clusterRoles | sortAlpha }}
{{- range $_, $roleKey := $clusterRoleKeys }}
{{- $role := index $.CurrentApp.clusterRoles $roleKey }}
{{ include "apps-service-accounts._renderRoleAndBinding" (list $ $.CurrentApp $roleKey $role "ClusterRole" "ClusterRoleBinding" false) }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
