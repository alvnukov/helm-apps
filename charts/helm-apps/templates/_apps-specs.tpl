{{- define "apps-specs.containers.volumes" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- with $relativeScope }}
{{- $volumes := include "apps-compat.renderListResolved" (list $ . "volumes" .volumes) | trim }}
{{- $hadContainer := hasKey $ "CurrentContainer" }}
{{- $previousContainer := $.CurrentContainer }}
{{- $hadContainersType := hasKey $.CurrentApp "_currentContainersType" }}
{{- $previousContainersType := $.CurrentApp._currentContainersType }}
{{- range $containersType := list "initContainers" "containers" }}
{{- range $containerName, $container := index $.CurrentApp $containersType }}
{{- if include "fl.isTrue" (list $ $container $container.enabled) }}
{{- $_ := set $container "name" $containerName }}
{{- $_ = set $ "CurrentContainer" $container }}
{{- $_ = set $.CurrentApp "_currentContainersType" $containersType }}
{{- $volumes = list $volumes (include "apps-compat.renderListResolved" (list $ $container "volumes" $container.volumes) | trim) | join "\n" | trim }}
{{- end }}
{{- end }}
{{- end }}
{{- if $hadContainer }}
{{- $_ := set $ "CurrentContainer" $previousContainer }}
{{- else }}
{{- $_ := unset $ "CurrentContainer" }}
{{- end }}
{{- if $hadContainersType }}
{{- $_ := set $.CurrentApp "_currentContainersType" $previousContainersType }}
{{- else }}
{{- $_ := unset $.CurrentApp "_currentContainersType" }}
{{- end }}
{{- $volumes = list $volumes (include "apps-helpers.generateVolumes" (list $ .) | trim) | join "\n" | trim }}
{{- if $volumes }}
{{- $names := dict }}
{{- range $volume := fromYamlArray $volumes }}
{{- if and (kindIs "map" $volume) (hasKey $volume "name") }}
{{- $name := toString $volume.name }}
{{- if hasKey $names $name }}
{{- include "apps-utils.error" (list $ "E_VOLUME_NAME_CONFLICT" (printf "duplicate pod volume name '%s'" $name) "use distinct names across app volumes, container volumes and managed config/secret volumes" "docs/reference-values.md#param-containers") }}
{{- end }}
{{- $_ := set $names $name true }}
{{- end }}
{{- end }}
{{ $volumes | nindent 0 }}
{{- end }}
{{- $_ := set . "__specName__" "volumes"}}
{{- end }}
{{- end }}

{{- define "apps-specs.selector" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- with $relativeScope }}
{{- $selector := include "fl.value" (list $ . .selector) }}
matchLabels:
{{- if empty $selector }}
{{- include "fl.generateSelectorLabels" (list $ . .name) | nindent 2 }}
{{- else }}
{{- $selector | nindent 2}}
{{- end }}
{{- $_ := set . "__specName__" "selector" }}
{{- end }}
{{- end }}

{{- define "apps-specs.serviceName" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- with $relativeScope }}
{{- include "fl.value" (list $ . .service.name) }}
{{- $_ := set . "__specName__" "serviceName"}}
{{- end }}
{{- end }}

{{- define "apps-specs.volumeClaimTemplates" }}
{{- $ := index . 0 }}
{{- $relativeScope := index . 1 }}
{{- with $relativeScope }}
{{- include "apps-compat.renderListResolved" (list $ . "volumeClaimTemplates" .volumeClaimTemplates) | nindent 0 }}
{{- /* Loop through containers to generate Pod volumes */ -}}
{{- range $_, $containersType := list "initContainers" "containers" }}
{{- range $_containerName, $_container := index $.CurrentApp $containersType }}
{{- if include "fl.isTrue" (list $ . .enabled) }}
{{- $_ := set . "name" $_containerName }}
{{- $_ = set $ "CurrentContainer" $_container }}
{{- range $persistantVolumeName, $persistantVolume := .persistantVolumes }}
{{- $pvcName := print $persistantVolumeName "-" $containersType "-" $.CurrentApp.name "-" $.CurrentContainer.name "-" $persistantVolume.mountPath | include "fl.formatStringAsDNSLabel" }}
  - metadata:
    name: {{ $pvcName }}
  spec:
    accessModes:{{ include "fl.value" (list $ . $persistantVolume.accessModes) | default "\n- ReadWriteOnce" | trim | nindent 4 }}
    resources:
      requests:
        storage: {{ include "fl.value" (list $ . $persistantVolume.size) }}
    storageClassName: {{ include "fl.value" (list $ . $persistantVolume.storageClass) }}
    volumeMode: Filesystem
{{- end }}
{{- end }}
{{- end }}
{{- end }}
{{- $_ := set . "__specName__" "volumeClaimTemplates"}}
{{- end }}
{{- end }}
