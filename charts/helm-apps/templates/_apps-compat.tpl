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
