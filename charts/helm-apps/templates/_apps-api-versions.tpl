{{- define "apps-api-versions.cronJob" -}}
{{- if or (.Capabilities.APIVersions.Has "batch/v1/CronJob") (semverCompare ">=1.21-0" .Capabilities.KubeVersion.GitVersion) -}}
batch/v1
{{- else -}}
batch/v1beta1
{{- end -}}
{{- end -}}

{{- define "apps-api-versions.podDisruptionBudget" -}}
{{- if or (.Capabilities.APIVersions.Has "policy/v1/PodDisruptionBudget") (semverCompare ">=1.21-0" .Capabilities.KubeVersion.GitVersion) -}}
policy/v1
{{- else -}}
policy/v1beta1
{{- end -}}
{{- end -}}

{{- define "apps-api-versions.horizontalPodAutoscaler" -}}
{{- if or (.Capabilities.APIVersions.Has "autoscaling/v2/HorizontalPodAutoscaler") (semverCompare ">=1.23-0" .Capabilities.KubeVersion.GitVersion) -}}
autoscaling/v2
{{- else -}}
autoscaling/v2beta2
{{- end -}}
{{- end -}}

{{- define "apps-api-versions.verticalPodAutoscaler" -}}
{{- if .Capabilities.APIVersions.Has "autoscaling.k8s.io/v1/VerticalPodAutoscaler" -}}
autoscaling.k8s.io/v1
{{- else if .Capabilities.APIVersions.Has "autoscaling.k8s.io/v1beta2/VerticalPodAutoscaler" -}}
autoscaling.k8s.io/v1beta2
{{- else -}}
autoscaling.k8s.io/v1
{{- end -}}
{{- end -}}
