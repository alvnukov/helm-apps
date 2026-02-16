{{- define "apps-network-policies" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
  {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-network-policies" "name" "apps-network-policies") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
  {{- end -}}
{{- end -}}

{{- define "apps-network-policies.render" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- $type := include "fl.value" (list $ . .type) | default "kubernetes" }}
{{- $apiVersion := include "fl.value" (list $ . .apiVersion) }}
{{- $kind := include "fl.value" (list $ . .kind) }}
{{- if not $apiVersion }}
  {{- if eq $type "kubernetes" }}
    {{- $apiVersion = "networking.k8s.io/v1" }}
  {{- else if eq $type "cilium" }}
    {{- $apiVersion = "cilium.io/v2" }}
  {{- else if eq $type "calico" }}
    {{- $apiVersion = "projectcalico.org/v3" }}
  {{- else }}
    {{- fail (printf "apps-network-policies.%s: set apiVersion/kind for unsupported type=%s" $.CurrentApp.name $type) }}
  {{- end }}
{{- end }}
{{- if not $kind }}
  {{- if eq $type "kubernetes" }}
    {{- $kind = "NetworkPolicy" }}
  {{- else if eq $type "cilium" }}
    {{- $kind = "CiliumNetworkPolicy" }}
  {{- else if eq $type "calico" }}
    {{- $kind = "NetworkPolicy" }}
  {{- else }}
    {{- fail (printf "apps-network-policies.%s: set apiVersion/kind for unsupported type=%s" $.CurrentApp.name $type) }}
  {{- end }}
{{- end }}
apiVersion: {{ $apiVersion }}
kind: {{ $kind }}
{{- include "apps-helpers.metadataGenerator" (list $ .) }}
spec:
  {{- if include "fl.value" (list $ . .spec) }}
  {{- include "apps-compat.renderRaw" (list $ . .spec) | trim | nindent 2 }}
  {{- else if eq $type "kubernetes" }}
  {{- with include "fl.value" (list $ . .podSelector) | trim }}
  podSelector:
    {{- . | nindent 4 }}
  {{- else }}
  podSelector:
    matchLabels:
{{- include "fl.generateSelectorLabels" (list $ . $.CurrentApp.name) | trim | nindent 6 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .policyTypes) }}
  policyTypes:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .ingress) }}
  ingress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .egress) }}
  egress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- else if eq $type "cilium" }}
  {{- with include "fl.value" (list $ . .endpointSelector) | trim }}
  endpointSelector:
    {{- . | nindent 4 }}
  {{- else }}
  endpointSelector:
    matchLabels:
{{- include "fl.generateSelectorLabels" (list $ . $.CurrentApp.name) | trim | nindent 6 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .ingress) }}
  ingress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .egress) }}
  egress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .ingressDeny) }}
  ingressDeny:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .egressDeny) }}
  egressDeny:
  {{- . | nindent 2 }}
  {{- end }}
  {{- else if eq $type "calico" }}
  {{- with include "fl.value" (list $ . .selector) }}
  selector: {{ . | quote }}
  {{- else }}
  selector: {{ print "app == '" $.CurrentApp.name "'" | quote }}
  {{- end }}
  {{- with include "fl.value" (list $ . .types) }}
  types:
  {{- . | nindent 2 }}
  {{- else }}
  {{- if or (include "fl.value" (list $ . .ingress)) (include "fl.value" (list $ . .egress)) }}
  types:
    {{- if include "fl.value" (list $ . .ingress) }}
    - Ingress
    {{- end }}
    {{- if include "fl.value" (list $ . .egress) }}
    - Egress
    {{- end }}
  {{- end }}
  {{- end }}
  {{- with include "fl.value" (list $ . .ingress) }}
  ingress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "fl.value" (list $ . .egress) }}
  egress:
  {{- . | nindent 2 }}
  {{- end }}
  {{- else }}
  {{- fail (printf "apps-network-policies.%s: unsupported type=%s (use kubernetes|cilium|calico or set apiVersion/kind+spec)" $.CurrentApp.name $type) }}
  {{- end }}
  {{- with include "apps-compat.renderRaw" (list $ . .extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
{{- end }}
{{- end }}
