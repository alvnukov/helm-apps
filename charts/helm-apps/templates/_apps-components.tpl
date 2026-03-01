{{- define "apps-components.verticalPodAutoscaler" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- $verticalPodAutoscaler := index . 2 }}
{{- $kind := index . 3 }}
{{- $targetRefApiVersion := "apps/v1" }}
{{- if eq $kind "CronJob" }}
{{- $targetRefApiVersion = include "apps-api-versions.cronJob" $ }}
{{- else if eq $kind "Job" }}
{{- $targetRefApiVersion = "batch/v1" }}
{{- end }}
{{- include "apps-utils.enterScope" (list $ "verticalPodAutoscaler") }}
{{- if $verticalPodAutoscaler }}
{{- if include "fl.isTrue" (list $ . $verticalPodAutoscaler.enabled) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: {{ include "apps-api-versions.verticalPodAutoscaler" $ }}
kind: VerticalPodAutoscaler
{{- include "apps-helpers.metadataGenerator" (list $ $verticalPodAutoscaler ) }}
spec:
  targetRef:
    apiVersion: {{ $targetRefApiVersion | quote }}
    kind: {{ $kind }}
    name: {{ $.CurrentApp.name | quote }}
  updatePolicy:
    updateMode: {{ include "fl.valueQuoted" (list $ . $verticalPodAutoscaler.updateMode) | default (print "Off" | quote )}}
{{- if include "fl.value" (list $ . $verticalPodAutoscaler.resourcePolicy) }}
  resourcePolicy: {{- include "fl.value" (list $ . $verticalPodAutoscaler.resourcePolicy) | trim | nindent 4 }}
{{- else }}
  resourcePolicy: {}
{{- end }}
{{- with include "apps-compat.renderRaw" (list $ . $verticalPodAutoscaler.extraSpec) | trim }}
  {{- . | nindent 2 }}
{{- end }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}

{{- define "apps-components.cerificate" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- with $RelatedScope }}
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: {{ include "fl.value" (list $ . .name) }}
spec:
  secretName: {{ include "fl.value" (list $ . .name) }}
  issuerRef:
    kind: ClusterIssuer
    name: {{ include "fl.valueQuoted" (list $ . .clusterIssuer) | default "letsencrypt" }}
  dnsNames:
  {{- with .host }}
  - {{ include "fl.valueQuoted" (list $ $RelatedScope .) }}
  {{- end }}
  {{- with .hosts }}
  {{- include "fl.value" (list $ $RelatedScope .) | nindent 2 }}
  {{- end }}
{{- end }}
{{- end }}

{{- define "apps-components.podDisruptionBudget" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- $podDisruptionBudget := index . 2 }}
{{- include "apps-utils.enterScope" (list $ "podDisruptionBudget") }}

{{- with $podDisruptionBudget }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: {{ include "apps-api-versions.podDisruptionBudget" $ }}
kind: PodDisruptionBudget
{{- include "apps-helpers.metadataGenerator" (list $ $podDisruptionBudget) }}
spec:
  selector:
    matchLabels:
{{- if empty (include "fl.value" (list $ . $.CurrentApp.selector)) }}
{{- include "fl.generateSelectorLabels" (list $ . $.CurrentApp.name) | trim | nindent 6 }}
{{- else }}
{{- $.CurrentApp.selector | trim | nindent 6 }}
{{- end }}
{{- with include "fl.value" (list $ . .maxUnavailable) }}
  maxUnavailable: {{ . }}
{{- end }}
{{- with include "fl.value" (list $ . .minAvailable) }}
  minAvailable: {{ . }}
{{- end }}
  {{- with include "apps-compat.renderRaw" (list $ . .extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}

{{- define "apps-components.service" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- $service := index . 2 }}
{{- if $service }}
{{- if include "fl.isTrue" (list $ . $service.enabled) }}
{{- $servicePortsRaw := include "apps-compat.renderRawResolved" (list $ . $service.ports) | trim -}}
{{- if or $servicePortsRaw (and (kindIs "slice" $service.ports) (gt (len $service.ports) 0)) }}
{{- include "apps-utils.enterScope" (list $ "service") }}
{{- include "apps-utils.printPath" $ -}}
{{- if empty (include "fl.value" (list $ . $service.selector)) }}
{{- $_ := set $service "selector" (include "fl.generateSelectorLabels" (list $ . $.CurrentApp.name) | trim) }}
{{- end }}
{{- if include "fl.isTrue" (list $ . $service.headless) }}
{{- $_ := set $service "clusterIP" "None" }}
{{- end }}
{{- include "apps-components._service" (list $ $service) }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}

{{- define "apps-components._service" }}
{{- $ := index . 0 -}}
{{- $RelatedScope := index . 1 -}}
{{- include "apps-compat.normalizeServiceSpec" (list $ $RelatedScope) -}}
apiVersion: v1
kind: Service
{{- include "apps-helpers.metadataGenerator" (list $ $RelatedScope) }}
spec:
  {{- /* https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.24/#service-v1-core */ -}}
  {{- $specs := dict -}}
  {{- $_ := set $specs "Bools" (list "publishNotReadyAddresses" "allocateLoadBalancerNodePorts") -}}
  {{- $_ = set $specs "Lists" (list "clusterIPs" "externalIPs" "ipFamilies" "loadBalancerSourceRanges" "ports") -}}
  {{- $_ = set $specs "Strings" (list "externalName" "externalTrafficPolicy" "internalTrafficPolicy" "ipFamilyPolicy" "loadBalancerClass" "loadBalancerIP" "sessionAffinity" "type" "clusterIP") -}}
  {{- $_ = set $specs "Numbers" (list "healthCheckNodePort") -}}
  {{- $_ = set $specs "Maps" (list "sessionAffinityConfig" "selector") -}}
  {{- with include "apps-utils.generateSpecs" (list $ $RelatedScope $specs) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  {{- with include "apps-compat.renderRaw" (list $ $RelatedScope $RelatedScope.extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
{{- end }}


{{- define "apps-components.horizontalPodAutoscaler" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
{{- include "apps-utils.enterScope" (list $ "horizontalPodAutoscaler") }}
{{- $kind := index . 2 }}
{{- with $RelatedScope }}
{{- if $.CurrentApp.horizontalPodAutoscaler }}
{{- if include "fl.isTrue" (list $ . $.CurrentApp.horizontalPodAutoscaler.enabled) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: {{ include "apps-api-versions.horizontalPodAutoscaler" $ }}
kind: HorizontalPodAutoscaler
{{- include "apps-helpers.metadataGenerator" (list $ $.CurrentApp.horizontalPodAutoscaler ) }}
spec:
  minReplicas: {{ include "fl.value" (list $ . $.CurrentApp.horizontalPodAutoscaler.minReplicas) }}
  maxReplicas: {{ include "fl.value" (list $ . $.CurrentApp.horizontalPodAutoscaler.maxReplicas) }}
  behavior: {{- include "fl.value" (list $ . $.CurrentApp.horizontalPodAutoscaler.behavior) | trim | nindent 4 }}
  scaleTargetRef:
    apiVersion: apps/v1
    kind: {{ $kind }}
    name: {{ $.CurrentApp.name | quote }}
  metrics:
{{- $hpaMetrics := $.CurrentApp.horizontalPodAutoscaler.metrics }}
{{- if not $hpaMetrics }}
{{- include "apps-utils.error" (list $ "E_HPA_METRICS_REQUIRED" (printf "horizontalPodAutoscaler.metrics is required for app '%s'" $.CurrentApp.name) "set metrics as YAML string or map" "docs/reference-values.md#param-hpa-metrics") }}
{{- else if kindIs "string" $hpaMetrics }}
{{- print (include "fl.value" (list $ . $hpaMetrics)) | nindent 2 }}
{{- else if kindIs "map" $hpaMetrics }}
{{- include "apps-helpers.generateHPAMetrics" (list $ $RelatedScope) | trim | nindent 2 }}
{{- else }}
{{- include "apps-utils.error" (list $ "E_HPA_METRICS_TYPE" (printf "horizontalPodAutoscaler.metrics has unsupported type for app '%s'" $.CurrentApp.name) "use string (raw YAML) or map" "docs/reference-values.md#param-hpa-metrics") }}
{{- end }}
  {{- with include "apps-compat.renderRaw" (list $ . $.CurrentApp.horizontalPodAutoscaler.extraSpec) | trim }}
  {{- . | nindent 2 }}
  {{- end }}

{{- range $_customMetricResourceName, $_customMetricResource := $.CurrentApp.horizontalPodAutoscaler.customMetricResources }}
{{- include "apps-utils.enterScope" (list $ $_customMetricResourceName) }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- $_ := set . "name" $_customMetricResourceName }}
{{- $currentApp := $.CurrentApp}}
{{- $_ = set $ "CurrentApp" . }}
---
{{- include "apps-utils.printPath" $ -}}
{{- include "apps-deckhouse-metrics.render" $ }}
{{- $_ = set $ "CurrentApp" $currentApp }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}

{{- define "apps-components.generateConfigMapsAndSecrets" }}
{{- $ := . }}
{{- /* Loop through containers to generate ConfigMaps and Secrets */ -}}
{{- range $_, $containersType := list "initContainers" "containers" }}
{{- include "apps-utils.enterScope" (list $ $containersType) }}
{{- range $_containerName, $_container := index $.CurrentApp $containersType }}
{{- include "apps-utils.enterScope" (list $ $_containerName) }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- $_ := set . "name" $_containerName }}
{{- $_ = set $ "CurrentContainer" $_container }}
{{- /* ConfigMaps created by "configFiles:" option */ -}}
{{- include "apps-utils.enterScope" (list $ "configFiles") }}
{{- range $configFileName, $configFile := .configFiles }}
{{- if include "fl.value" (list $ . .content) }}
{{- include "apps-utils.enterScope" (list $ $configFileName) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: v1
kind: ConfigMap
metadata:
  name: {{ print "config-" $containersType "-" $.CurrentApp.name "-" $.CurrentContainer.name "-" $configFileName | include "fl.formatStringAsDNSLabel" | quote }}
  {{- with  include "apps-helpers.generateAnnotations" (list $ .) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  labels:{{ include "fl.generateLabels" (list $ . $.CurrentApp.name) | trim | nindent 4 }}
data:
  {{ $configFileName | quote }}: | {{ include "fl.value" (list $ . .content) | trim | nindent 4 }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- /* ConfigMaps created by "configFilesYAML:" option */ -}}
{{- include "apps-utils.enterScope" (list $ "configFilesYAML") }}
{{- range $configFileName, $configFile := .configFilesYAML }}
{{- if kindIs "map" .content }}
{{- include "apps-utils.enterScope" (list $ $configFileName) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: v1
kind: ConfigMap
metadata:
  name: {{ print "config-yaml-" $containersType "-" $.CurrentApp.name "-" $.CurrentContainer.name "-" $configFileName | include "fl.formatStringAsDNSLabel" | quote }}
  {{- with  include "apps-helpers.generateAnnotations" (list $ .) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  labels:{{ include "fl.generateLabels" (list $ . $.CurrentApp.name) | trim | nindent 4 }}
data:
{{- include "apps-helpers.generateConfigYAML" (list $ .content .content "content") }}
  {{ $configFileName | quote }}: | {{ toYaml .content | trim | nindent 4 }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- /* Secrets created by "secretConfigFiles:" option */ -}}
{{- range $secretConfigFileName, $secretConfigFile := .secretConfigFiles }}
{{- if include "fl.value" (list $ . .content) }}
{{- include "apps-utils.enterScope" (list $ $secretConfigFileName) }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: v1
kind: Secret
metadata:
  name: {{ print "config-" $containersType "-" $.CurrentApp.name "-" $.CurrentContainer.name "-" $secretConfigFileName | include "fl.formatStringAsDNSLabel" | quote }}
  {{- with  include "apps-helpers.generateAnnotations" (list $ .) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  labels:{{ include "fl.generateLabels" (list $ . $.CurrentApp.name) | trim | nindent 4 }}
type: Opaque
data:
  {{ $secretConfigFileName | quote }}: {{ include "fl.value" (list $ . .content) | b64enc | quote }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- /* Secret created by "secretEnvVars:" option */ -}}
{{- if include "fl.generateSecretEnvVars" (list $ . .secretEnvVars) }}
{{- include "apps-utils.enterScope" (list $ "secretEnvVars") }}
{{- include "apps-utils.printPath" $ -}}
apiVersion: v1
kind: Secret
metadata:
  name: {{ print "envs-" $containersType "-" $.CurrentApp.name "-" .name | include "fl.formatStringAsDNSLabel" | quote }}
  {{- with  include "apps-helpers.generateAnnotations" (list $ .) | trim }}
  {{- . | nindent 2 }}
  {{- end }}
  labels:{{ include "fl.generateLabels" (list $ . $.CurrentApp.name) | trim | nindent 4 }}
type: Opaque
data:{{ include "fl.generateSecretEnvVars" (list $ . .secretEnvVars) | trim | nindent 2 }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- include "apps-utils.leaveScope" $ }}
{{- end }}
{{- end }}

{{- define "apps-components.generate-config-checksum" }}
{{- $ := index . 0 }}
{{- $RelatedScope := index . 1 }}
  {{- /* Loop through containers to generate Pod volumes */ -}}
{{- $allConfigMaps := "" }}
{{- range $_, $containersType := list "initContainers" "containers" }}
{{- range $_containerName, $_container := index $.CurrentApp $containersType }}
{{- $_ := set $ "CurrentContainer" . }}
{{- if hasKey . "enabled" }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- $allConfigMaps = print $allConfigMaps (include "apps-components._generate-config-checksum" $) }}
{{- end }}
{{- else }}
{{- $allConfigMaps = print $allConfigMaps (include "apps-components._generate-config-checksum" $) }}
{{- end }}
{{- end }}

{{- end }}
{{- printf "checksum/config: '%s'" ($allConfigMaps | sha256sum) }}
{{- end }}

{{- define "apps-components._generate-config-checksum" }}
{{- $ := . }}
{{- with $.CurrentApp }}
{{- range $_, $configFile := $.CurrentContainer.configFiles }}
{{- print (include "fl.value" (list $ . $configFile.content)) }}
{{- end }}
{{- range $_, $configFile :=  $.CurrentContainer.secretConfigFiles }}
{{- print (include "fl.value" (list $ . $configFile.content)) }}
{{- end }}
{{- range $_, $configFile :=  $.CurrentContainer.configFilesYAML }}

{{- include "apps-helpers.generateConfigYAML" (list $ $configFile.content $configFile.content "content") }}
{{- $configFile.content | toYaml }}
{{- end }}
{{- end }}
{{- end }}
