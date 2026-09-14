{{- /*
Group/version selection for objects whose API moved between Kubernetes releases.

Each helper prefers what the cluster actually serves, and falls back to the
version implied by the target Kubernetes version (see apps-compat.kubeVersion,
which honours global.compat.kubeVersion). The version thresholds are the
releases that introduced the stable group/version:
  batch/v1 CronJob            1.21
  policy/v1 PDB               1.21
  autoscaling/v2 HPA          1.23
The beta fallbacks below were removed in Kubernetes 1.25 (batch/v1beta1,
policy/v1beta1) and 1.26 (autoscaling/v2beta2), so they are only ever reached
for clusters older than that.
*/ -}}

{{- define "apps-api-versions.cronJob" -}}
{{- if or (.Capabilities.APIVersions.Has "batch/v1/CronJob") (include "apps-compat.kubeAtLeast" (list . "1.21")) -}}
batch/v1
{{- else -}}
batch/v1beta1
{{- end -}}
{{- end -}}

{{- define "apps-api-versions.podDisruptionBudget" -}}
{{- if or (.Capabilities.APIVersions.Has "policy/v1/PodDisruptionBudget") (include "apps-compat.kubeAtLeast" (list . "1.21")) -}}
policy/v1
{{- else -}}
policy/v1beta1
{{- end -}}
{{- end -}}

{{- define "apps-api-versions.horizontalPodAutoscaler" -}}
{{- if or (.Capabilities.APIVersions.Has "autoscaling/v2/HorizontalPodAutoscaler") (include "apps-compat.kubeAtLeast" (list . "1.23")) -}}
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

{{- /*
Strimzi, not Kubernetes: kafka.strimzi.io/v1beta1 was dropped from the CRDs in
Strimzi 0.23, so v1beta2 is the default and v1beta1 is only used when the
cluster still serves it.
*/ -}}
{{- define "apps-api-versions.kafkaTopic" -}}
{{- if .Capabilities.APIVersions.Has "kafka.strimzi.io/v1beta2/KafkaTopic" -}}
kafka.strimzi.io/v1beta2
{{- else if .Capabilities.APIVersions.Has "kafka.strimzi.io/v1beta1/KafkaTopic" -}}
kafka.strimzi.io/v1beta1
{{- else -}}
kafka.strimzi.io/v1beta2
{{- end -}}
{{- end -}}
