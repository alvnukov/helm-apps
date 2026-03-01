{{- define "apps-ingresses" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 }}
    {{- if not (kindIs "invalid" $RelatedScope) }}
  {{- $_ := set $RelatedScope "__GroupVars__" (dict "type" "apps-ingresses" "name" "apps-ingresses") }}
  {{- include "apps-utils.renderApps" (list $ $RelatedScope) }}
  {{- end -}}
{{- end -}}

{{- define "apps-ingresses.render" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- $_ := set $ "CurrentIngress" . }}
{{- $ingressClass := include "fl.value" (list $ . .class) | trim }}
{{- $userAnnotations := include "fl.value" (list $ . .annotations) | trim }}
{{- if or (eq $userAnnotations "{}") (eq $userAnnotations "null") }}
{{- $userAnnotations = "" }}
{{- end }}
{{- $hasDexAuthAnnotations := false }}
{{- with .dexAuth }}
{{- if and (include "fl.isTrue" (list $ $.CurrentApp .enabled)) $.Values.werf }}
{{- $hasDexAuthAnnotations = true }}
{{- end }}
{{- end }}
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: {{ .name | quote }}
  {{- if or $ingressClass $userAnnotations $hasDexAuthAnnotations }}
  annotations:
    {{- with $ingressClass }}
    kubernetes.io/ingress.class: {{ . | quote }}
    {{- end }}
    {{- with $userAnnotations }}
    {{- . | nindent 4 }}
    {{- end }}
    {{- if $hasDexAuthAnnotations }}
    {{- with .dexAuth }}
    {{- include "apps-utils.enterScope" (list $ "dexAuth") }}
    nginx.ingress.kubernetes.io/auth-signin: https://$host/dex-authenticator/sign_in
    nginx.ingress.kubernetes.io/auth-url: https://{{ $.CurrentApp.name }}-dex-authenticator.{{ $.Values.werf.namespace }}.svc.{{ include "apps-utils.requiredValue" (list $ . "clusterDomain") }}/dex-authenticator/auth
    nginx.ingress.kubernetes.io/auth-response-headers: X-Auth-Request-User,X-Auth-Request-Email,Authorization
    {{- include "apps-utils.leaveScope" $ }}
    {{- end }}
    {{- end }}
  {{- end }}
  labels: {{- include "fl.generateLabels" (list $ . .name) | nindent 4 }}
spec:
  {{- if include "fl.value" (list $ . .ingressClassName) }}
  ingressClassName: {{ include "fl.value" (list $ . .ingressClassName) }}{{- end }}
  {{- if .tls }}
  {{- if include "fl.isTrue" (list $ . .tls.enabled) }}
  tls:
  {{- if (include "fl.value" (list $ . .tls.secret_name)) }}
  - secretName: {{ include "fl.value" (list $ . .tls.secret_name) }}
  {{- else }}
  - secretName: {{ include "fl.value" (list $ . .name) }}
  {{- end }}
  {{- end }}
  {{- end }}
  rules:
  - host: {{ include "fl.valueQuoted" (list $ . .host) }}
    http:
      paths: {{- include "fl.value" (list $ . .paths) | trim | nindent 6 }}
  {{- with include "apps-compat.renderRaw" (list $ . .extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
{{- if .tls }}
{{- if include "fl.isTrue" (list $ . .tls.enabled) }}
{{- if not (include "fl.value" (list $ . .tls.secret_name)) }}
{{- include "apps-utils.enterScope" (list $ "tls") }}
{{- include "apps-utils.printPath" $ -}}
{{- include "apps-components.cerificate" (list $ .) }}
{{- include "apps-utils.leaveScope" $ }}
{{- end -}}

{{- with .dexAuth }}
{{- if (include "fl.isTrue" (list $ $.CurrentApp .enabled)) }}
{{- $_ := set $.CurrentApp "applicationDomain" $.CurrentApp.host }}
{{- if $.CurrentApp.class }}
{{- $_ = set $.CurrentApp "applicationIngressClassName" $.CurrentApp.class }}
{{- else }}
{{- $_ = set $.CurrentApp "applicationIngressClassName" (include "apps-utils.requiredValue" (list $ $.CurrentApp "ingressClassName")) }}
{{- end }}
{{- if (include "fl.value" (list $ . $.CurrentApp.tls.secret_name)) }}
{{- $_ = set $.CurrentApp "applicationIngressCertificateSecretName" (include "fl.value" (list $ $.CurrentApp $.CurrentApp.tls.secret_name)) }}
{{- else }}
{{- $_ = set $.CurrentApp "applicationIngressCertificateSecretName" (include "fl.value" (list $ $.CurrentApp $.CurrentApp.name)) }}
{{- end }}
---
{{- include "apps-dex-authenticators.render" $ }}
{{- end }}
{{- end }}
{{- end }}

{{- end }}
{{- end }}
{{- end }}
