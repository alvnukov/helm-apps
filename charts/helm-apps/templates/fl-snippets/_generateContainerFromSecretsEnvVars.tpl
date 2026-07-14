{{- define "fl.generateContainerFromSecretsEnvVars" }}
  {{- $ := index . 0 }}
  {{- $relativeScope := index . 1 }}
  {{- $envsFromSecret := index . 2 }}
{{- range $secretName, $secretVars := $envsFromSecret }}
  {{- range $envVarName, $envNameInSecret := $secretVars }}
    {{- $resolvedSecretKey := include "apps.value" (list $ $relativeScope $envNameInSecret $envVarName) }}
    {{- if ne $resolvedSecretKey "" }}
      {{- if $.Values.global.configFlantLibVariableUppercaseEnvs }}
        {{- $envVarName = upper $envVarName }}
      {{- end }}
- name: {{ $envVarName | quote }}
  valueFrom:
    secretKeyRef:
      name: {{ $secretName | quote }}
      key: {{ $resolvedSecretKey | quote }}
    {{- end }}
  {{- end }}
{{- end }}
{{- end }}


