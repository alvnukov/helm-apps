{{- define "apps-system.serviceAccount" }}
{{- $ := . }}
{{- with $.CurrentApp.serviceAccount }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- include "apps-utils.enterScope" (list $ "serviceAccount") }}
{{- if not (hasKey . "name") }}
{{- $_ := set . "name" $.CurrentApp.name }}
{{- end }}
{{- $serviceAccountName := include "fl.value" (list $ . .name) }}
{{- $_ := set $.CurrentApp "serviceAccountName" $serviceAccountName }}
---
apiVersion: v1
kind: ServiceAccount
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
{{- if hasKey . "clusterRole" }}
{{- if include "apps-compat.forbidLegacyServiceAccountClusterRole" (list $) }}
{{- include "apps-utils.error" (list $ "E_LEGACY_SERVICE_ACCOUNT_CLUSTER_ROLE_FORBIDDEN" "serviceAccount.clusterRole is forbidden by validation policy" "migrate to apps-service-accounts or disable global.validation.forbidLegacyServiceAccountClusterRole" "docs/reference-values.md#param-serviceaccount") }}
{{- end }}
{{- include "apps-utils.enterScope" (list $ "clusterRole") }}
{{- $roleName := include "apps-utils.requiredValue" (list $ .clusterRole "name") }}
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
{{- include "apps-helpers.metadataGenerator" (list $ .clusterRole) }}
rules:
{{- include "apps-utils.requiredValue" (list $ .clusterRole "rules") | nindent 2 }}
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
{{- include "apps-helpers.metadataGenerator" (list $ .clusterRole) }}
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: ClusterRole
  name:  {{ $roleName }}
subjects:
- kind: ServiceAccount
  name: {{ $serviceAccountName }}
  namespace: {{ $.Release.Namespace }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- end }}
