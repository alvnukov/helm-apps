# TODO:
{{- define "apps-infra" }}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 -}}
  {{- include "apps-utils.enterScope" (list $ "apps-infra") }}
  {{- if hasKey $RelatedScope "node-users"}}
  {{- include "apps-infra.node-users" (list $ (index $RelatedScope "node-users")) }}
  {{- end }}
  {{- if hasKey $RelatedScope "node-groups"}}
  {{- include "apps-infra.node-groups" (list $ (index $RelatedScope "node-groups")) }}
  {{- end }}
  {{- include "apps-utils.leaveScope" $ }}
{{- end }}

{{- define "apps-infra.node-users"}}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 -}}
   {{- include "apps-utils.enterScope" (list $ "node-users") }}
   {{- range $_appName, $_app := omit $RelatedScope  "global" "enabled" "_include" "__GroupVars__" -}}
   {{- include "apps-utils.enterScope" (list $ $_appName) }}
   {{- $_ := set . "name" $_appName }}
   {{- $_ = set $ "CurrentApp" $_app }}
{{- include "apps-utils.preRenderHooks" $ }}
   {{- if include "fl.isTrue" (list $ . .enabled) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: deckhouse.io/v1
kind: NodeUser
metadata:
  name: {{ .name | quote }}
  {{- with include "fl.value" (list $ . .annotations) | trim }}
  annotations:
    {{- . | nindent 4 }}
  {{- end }}
  {{- $addEnvLabel := false }}
  {{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "labels") (kindIs "map" $.Values.global.labels) (hasKey $.Values.global.labels "addEnv") }}
  {{- if include "fl.isTrue" (list $ . $.Values.global.labels.addEnv) }}
  {{- $addEnvLabel = true }}
  {{- end }}
  {{- end }}
  {{- $labels := include "fl.value" (list $ . .labels) | trim }}
  {{- if or $addEnvLabel $labels }}
  labels:
    {{- if $addEnvLabel }}
    app.kubernetes.io/environment: {{ include "fl.currentEnv" (list $) | quote }}
    {{- end }}
    {{- with $labels }}
    {{- . | nindent 4 }}
    {{- end }}
  {{- end }}
spec:
{{- $specs := dict -}}
{{- $_ := set $specs "Lists" (list "extraGroups" "nodeGroups" "sshPublicKeys") -}}
{{- $_ = set $specs "Maps" (list) -}}
{{- $_ = set $specs "Strings" (list "sshPublicKey" "passwordHash") -}}
{{- $_ = set $specs "Numbers" (list "uid") -}}
{{- $_ = set $specs "Bools" (list "isSudoer") -}}
{{- $_ = set $specs "Required" (list "uid") -}}
  {{- with include "apps-utils.generateSpecs" (list $ . $specs) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  {{- end }}
  {{- include "apps-utils.leaveScope" $ }}
  {{- end }}
  {{- include "apps-utils.leaveScope" $ }}
{{- end }}

{{- define "apps-infra.node-groups"}}
  {{- $ := index . 0 }}
  {{- $RelatedScope := index . 1 -}}
   {{- include "apps-utils.enterScope" (list $ "node-groups") }}
   {{- range $_appName, $_app := omit $RelatedScope  "global" "enabled" "_include" "__GroupVars__" -}}
   {{- include "apps-utils.enterScope" (list $ $_appName) }}
   {{- $_ := set . "name" $_appName }}
   {{- $_ = set $ "CurrentApp" $_app }}
   {{- if ._preRenderHook }}
    {{- $_ := include "fl.value" (list $ . ._preRenderHook) }}
   {{- end }}
   {{- if include "fl.isTrue" (list $ . .enabled) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: deckhouse.io/v1
kind: NodeGroup
metadata:
  name: {{ .name | quote }}
  {{- with include "fl.value" (list $ . .annotations) | trim }}
  annotations:
    {{- . | nindent 4 }}
  {{- end }}
  {{- $addEnvLabel := false }}
  {{- if and (hasKey $.Values "global") (kindIs "map" $.Values.global) (hasKey $.Values.global "labels") (kindIs "map" $.Values.global.labels) (hasKey $.Values.global.labels "addEnv") }}
  {{- if include "fl.isTrue" (list $ . $.Values.global.labels.addEnv) }}
  {{- $addEnvLabel = true }}
  {{- end }}
  {{- end }}
  {{- $labels := include "fl.value" (list $ . .labels) | trim }}
  {{- if or $addEnvLabel $labels }}
  labels:
    {{- if $addEnvLabel }}
    app.kubernetes.io/environment: {{ include "fl.currentEnv" (list $) | quote }}
    {{- end }}
    {{- with $labels }}
    {{- . | nindent 4 }}
    {{- end }}
  {{- end }}
spec:
{{- $specs := dict -}}
{{- $_ := set $specs "Lists" (list ) -}}
{{- $_ = set $specs "Maps" (list) -}}
{{- $_ = set $specs "Strings" (list ) -}}
{{- $_ = set $specs "Numbers" (list ) -}}
{{- $_ = set $specs "Bools" (list ) -}}
{{- $_ = set $specs "Required" (list ) -}}
  {{- with include "apps-utils.generateSpecs" (list $ . $specs) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  {{- end }}
  {{- include "apps-utils.leaveScope" $ }}
  {{- end }}
  {{- include "apps-utils.leaveScope" $ }}
{{- end }}
