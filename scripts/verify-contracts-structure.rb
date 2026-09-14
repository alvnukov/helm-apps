#!/usr/bin/env ruby
# frozen_string_literal: true

require 'optparse'
require 'yaml'

class AssertionError < StandardError; end

def assert!(condition, message)
  raise AssertionError, message unless condition
end

def assert_eq!(actual, expected, message)
  return if actual == expected

  raise AssertionError, "#{message}: expected #{expected.inspect}, got #{actual.inspect}"
end

def load_docs(path)
  raw = File.read(path)

  chunks = []
  current = []
  seen_api_version = false

  raw.each_line do |line|
    if line.strip == '---'
      unless current.empty?
        chunks << current.join
        current = []
        seen_api_version = false
      end
      next
    end

    if current.empty?
      # Ignore preamble comments/blank lines before first resource key.
      next if line.strip.empty? || line.start_with?('#')

      current << line
      seen_api_version = line.start_with?('apiVersion:')
      next
    end

    # Some render outputs may miss "---" between resources.
    # If a second top-level apiVersion appears within the same chunk,
    # treat it as a start of the next document.
    if line.start_with?('apiVersion:') && seen_api_version
      chunks << current.join
      current = [line]
      seen_api_version = true
      next
    end

    current << line
    seen_api_version ||= line.start_with?('apiVersion:')
  end

  chunks << current.join unless current.empty?

  docs = chunks.map { |chunk| YAML.load(chunk) }.compact.select { |doc| doc.is_a?(Hash) }
  assert!(!docs.empty?, "No YAML documents found in #{path}")
  docs
rescue Errno::ENOENT
  raise AssertionError, "File not found: #{path}"
rescue Psych::SyntaxError => e
  raise AssertionError, "Invalid YAML in #{path}: #{e.message}"
end

def find_docs(docs, kind: nil, name: nil, api_version: nil)
  docs.select do |doc|
    (kind.nil? || doc['kind'] == kind) &&
      (name.nil? || doc.dig('metadata', 'name') == name) &&
      (api_version.nil? || doc['apiVersion'] == api_version)
  end
end

def find_one!(docs, kind:, name:, api_version: nil)
  matches = find_docs(docs, kind: kind, name: name, api_version: api_version)
  assert!(!matches.empty?, "Missing #{kind}/#{name}#{api_version ? " apiVersion=#{api_version}" : ''}")
  assert!(matches.size == 1, "Expected single #{kind}/#{name}, got #{matches.size}")
  matches.first
end

def env_from_refs(deployment_doc)
  env_from = deployment_doc.dig('spec', 'template', 'spec', 'containers', 0, 'envFrom')
  return [] unless env_from.is_a?(Array)

  env_from.map do |entry|
    if entry.is_a?(Hash) && entry['configMapRef'].is_a?(Hash)
      "configMapRef:#{entry['configMapRef']['name']}"
    elsif entry.is_a?(Hash) && entry['secretRef'].is_a?(Hash)
      "secretRef:#{entry['secretRef']['name']}"
    else
      entry.inspect
    end
  end
end

def verify_required_entities!(docs)
  required = [
    ['Deployment', 'compat-modern-apis'],
    ['PodDisruptionBudget', 'compat-modern-apis'],
    ['Service', 'compat-modern-apis'],
    ['Job', 'compat-job-indexed'],
    ['StatefulSet', 'compat-stateful'],
    ['DaemonSet', 'compat-daemonset'],
    ['CronJob', 'compat-cron'],
    ['Service', 'compat-standalone-service'],
    ['LimitRange', 'compat-limit-range'],
    ['Certificate', 'compat-certificate'],
    ['DexAuthenticator', 'compat-dex-auth'],
    ['DexClient', 'compat-dex-client'],
    ['CustomPrometheusRules', 'compat-rules'],
    ['GrafanaDashboardDefinition', 'compat-dashboard'],
    ['KafkaTopic', 'compat-topic'],
    ['NodeUser', 'compat-user'],
    ['NodeGroup', 'compat-group']
  ]

  required.each do |kind, name|
    find_one!(docs, kind: kind, name: name)
  end

  kafka = find_docs(docs, kind: 'Kafka')
  assert!(!kafka.empty?, 'Missing Kafka resource for contracts scenario')
  assert!(kafka.any? { |doc| doc.dig('metadata', 'name').to_s.start_with?('compat-kafka-') }, 'Kafka name must start with compat-kafka-')
end

# Kubernetes API version gates.
#
# Every field below is emitted only at or above the Kubernetes release that
# introduced it in the API schema. The three renders bracket the gates:
#   1.20  - none of them exist yet
#   1.29  - everything introduced up to 1.28 exists, the 1.30 fields do not
#   modern - all of them exist
# See docs/operations.md#kubernetes-api-compatibility.
def verify_version_gates!(modern_docs, k129_docs, k120_docs)
  modern = find_one!(modern_docs, kind: 'Deployment', name: 'compat-modern-apis')
  modern_129 = find_one!(k129_docs, kind: 'Deployment', name: 'compat-modern-apis')
  modern_120 = find_one!(k120_docs, kind: 'Deployment', name: 'compat-modern-apis')

  # hostUsers: 1.25+
  assert_eq!(modern.dig('spec', 'template', 'spec', 'hostUsers'), false, 'modern hostUsers')
  assert_eq!(modern_129.dig('spec', 'template', 'spec', 'hostUsers'), false, 'k8s 1.29 hostUsers')
  assert_eq!(modern_120.dig('spec', 'template', 'spec', 'hostUsers'), nil, 'k8s 1.20 hostUsers must be absent')

  # schedulingGates: 1.26+
  assert_eq!(modern.dig('spec', 'template', 'spec', 'schedulingGates', 0, 'name'), 'compat.example/gate', 'modern schedulingGates[0].name')
  assert_eq!(modern_129.dig('spec', 'template', 'spec', 'schedulingGates', 0, 'name'), 'compat.example/gate', 'k8s 1.29 schedulingGates[0].name')
  assert_eq!(modern_120.dig('spec', 'template', 'spec', 'schedulingGates'), nil, 'k8s 1.20 schedulingGates must be absent')

  # container resizePolicy: 1.27+
  assert_eq!(modern.dig('spec', 'template', 'spec', 'containers', 0, 'resizePolicy', 0, 'resourceName'), 'cpu', 'modern resizePolicy[0].resourceName')
  assert_eq!(modern_129.dig('spec', 'template', 'spec', 'containers', 0, 'resizePolicy', 0, 'resourceName'), 'cpu', 'k8s 1.29 resizePolicy[0].resourceName')
  assert_eq!(modern_120.dig('spec', 'template', 'spec', 'containers', 0, 'resizePolicy'), nil, 'k8s 1.20 resizePolicy must be absent')

  # native sidecar (initContainer restartPolicy): 1.28+
  assert_eq!(modern.dig('spec', 'template', 'spec', 'initContainers', 0, 'restartPolicy'), 'Always', 'modern initContainer restartPolicy')
  assert_eq!(modern_129.dig('spec', 'template', 'spec', 'initContainers', 0, 'restartPolicy'), 'Always', 'k8s 1.29 initContainer restartPolicy')
  assert_eq!(modern_120.dig('spec', 'template', 'spec', 'initContainers', 0, 'restartPolicy'), nil, 'k8s 1.20 initContainer restartPolicy must be absent')

  # unhealthyPodEvictionPolicy: 1.26+
  pdb_modern = find_one!(modern_docs, kind: 'PodDisruptionBudget', name: 'compat-modern-apis')
  pdb_129 = find_one!(k129_docs, kind: 'PodDisruptionBudget', name: 'compat-modern-apis')
  pdb_120 = find_one!(k120_docs, kind: 'PodDisruptionBudget', name: 'compat-modern-apis')
  assert_eq!(pdb_modern.dig('spec', 'unhealthyPodEvictionPolicy'), 'AlwaysAllow', 'modern unhealthyPodEvictionPolicy')
  assert_eq!(pdb_129.dig('spec', 'unhealthyPodEvictionPolicy'), 'AlwaysAllow', 'k8s 1.29 unhealthyPodEvictionPolicy')
  assert_eq!(pdb_120.dig('spec', 'unhealthyPodEvictionPolicy'), nil, 'k8s 1.20 unhealthyPodEvictionPolicy must be absent')
  assert_eq!(pdb_modern['apiVersion'], 'policy/v1', 'modern PodDisruptionBudget apiVersion')
  assert_eq!(pdb_120['apiVersion'], 'policy/v1beta1', 'k8s 1.20 PodDisruptionBudget apiVersion')

  # trafficDistribution: 1.30+
  svc_modern = find_one!(modern_docs, kind: 'Service', name: 'compat-modern-apis')
  svc_129 = find_one!(k129_docs, kind: 'Service', name: 'compat-modern-apis')
  svc_120 = find_one!(k120_docs, kind: 'Service', name: 'compat-modern-apis')
  assert_eq!(svc_modern.dig('spec', 'trafficDistribution'), 'PreferClose', 'modern trafficDistribution')
  assert_eq!(svc_129.dig('spec', 'trafficDistribution'), nil, 'k8s 1.29 trafficDistribution must be absent')
  assert_eq!(svc_120.dig('spec', 'trafficDistribution'), nil, 'k8s 1.20 trafficDistribution must be absent')

  # Job: podFailurePolicy 1.25+, backoffLimitPerIndex/maxFailedIndexes/podReplacementPolicy 1.28+,
  # managedBy/successPolicy 1.30+.
  job_modern = find_one!(modern_docs, kind: 'Job', name: 'compat-job-indexed')
  job_129 = find_one!(k129_docs, kind: 'Job', name: 'compat-job-indexed')
  job_120 = find_one!(k120_docs, kind: 'Job', name: 'compat-job-indexed')

  # completionMode and JobSpec.suspend are 1.21+.
  assert_eq!(job_modern.dig('spec', 'completionMode'), 'Indexed', 'modern job completionMode')
  assert_eq!(job_modern.dig('spec', 'suspend'), false, 'modern job suspend')
  assert_eq!(job_120.dig('spec', 'completionMode'), nil, 'k8s 1.20 job completionMode must be absent')
  assert_eq!(job_120.dig('spec', 'suspend'), nil, 'k8s 1.20 job suspend must be absent')

  assert_eq!(job_modern.dig('spec', 'podFailurePolicy', 'rules', 0, 'action'), 'FailIndex', 'modern job podFailurePolicy')
  assert_eq!(job_129.dig('spec', 'podFailurePolicy', 'rules', 0, 'action'), 'FailIndex', 'k8s 1.29 job podFailurePolicy')
  assert_eq!(job_120.dig('spec', 'podFailurePolicy'), nil, 'k8s 1.20 job podFailurePolicy must be absent')

  %w[backoffLimitPerIndex maxFailedIndexes podReplacementPolicy].each do |field|
    assert!(!job_modern.dig('spec', field).nil?, "modern job #{field} must be present")
    assert!(!job_129.dig('spec', field).nil?, "k8s 1.29 job #{field} must be present")
    assert_eq!(job_120.dig('spec', field), nil, "k8s 1.20 job #{field} must be absent")
  end

  assert_eq!(job_modern.dig('spec', 'managedBy'), 'compat.example/job-controller', 'modern job managedBy')
  assert_eq!(job_modern.dig('spec', 'successPolicy', 'rules', 0, 'succeededCount'), 2, 'modern job successPolicy')
  %w[managedBy successPolicy].each do |field|
    assert_eq!(job_129.dig('spec', field), nil, "k8s 1.29 job #{field} must be absent")
    assert_eq!(job_120.dig('spec', field), nil, "k8s 1.20 job #{field} must be absent")
  end

  # StatefulSet ordinals: 1.26+; persistentVolumeClaimRetentionPolicy: 1.23+.
  sts_modern = find_one!(modern_docs, kind: 'StatefulSet', name: 'compat-stateful')
  sts_129 = find_one!(k129_docs, kind: 'StatefulSet', name: 'compat-stateful')
  sts_120 = find_one!(k120_docs, kind: 'StatefulSet', name: 'compat-stateful')
  assert_eq!(sts_modern.dig('spec', 'ordinals', 'start'), 1, 'modern statefulset ordinals.start')
  assert_eq!(sts_129.dig('spec', 'ordinals', 'start'), 1, 'k8s 1.29 statefulset ordinals.start')
  assert_eq!(sts_120.dig('spec', 'ordinals'), nil, 'k8s 1.20 statefulset ordinals must be absent')
  assert_eq!(sts_120.dig('spec', 'persistentVolumeClaimRetentionPolicy'), nil, 'k8s 1.20 statefulset persistentVolumeClaimRetentionPolicy must be absent')
  assert_eq!(sts_modern.dig('spec', 'progressDeadlineSeconds'), nil, 'statefulset progressDeadlineSeconds must never be emitted')

  # CronJob timeZone: 1.24+.
  cron_modern = find_one!(modern_docs, kind: 'CronJob', name: 'compat-cron')
  cron_129 = find_one!(k129_docs, kind: 'CronJob', name: 'compat-cron')
  cron_120 = find_one!(k120_docs, kind: 'CronJob', name: 'compat-cron')
  assert_eq!(cron_modern.dig('spec', 'timeZone'), 'Europe/Moscow', 'modern cronjob timeZone')
  assert_eq!(cron_129.dig('spec', 'timeZone'), 'Europe/Moscow', 'k8s 1.29 cronjob timeZone')
  assert_eq!(cron_120.dig('spec', 'timeZone'), nil, 'k8s 1.20 cronjob timeZone must be absent')
  assert_eq!(cron_modern['apiVersion'], 'batch/v1', 'modern CronJob apiVersion')
  assert_eq!(cron_120['apiVersion'], 'batch/v1beta1', 'k8s 1.20 CronJob apiVersion')

  # Strimzi KafkaTopic must use the group/version Strimzi still ships.
  kafka_topic = find_one!(modern_docs, kind: 'KafkaTopic', name: 'compat-topic')
  assert_eq!(kafka_topic['apiVersion'], 'kafka.strimzi.io/v1beta2', 'KafkaTopic apiVersion')
end

def verify_main!(paths)
  prod_docs = load_docs(paths[:production])
  dev_docs = load_docs(paths[:dev])
  strict_docs = load_docs(paths[:strict])
  k129_docs = load_docs(paths[:k129])
  k120_docs = load_docs(paths[:k120])
  k119_docs = load_docs(paths[:k119])
  kmodern_docs = load_docs(paths[:kmodern])

  merge_contract = find_one!(prod_docs, kind: 'ConfigMap', name: 'merge-contract')
  data = merge_contract['data'] || {}
  assert_eq!(data['A'], '2', 'merge-contract.data.A')
  assert_eq!(data['LOCAL'], 'ok', 'merge-contract.data.LOCAL')
  assert_eq!(data['key1'], 'value-1', 'merge-contract.data.key1')
  assert_eq!(data['key2'], 'local-value-2', 'merge-contract.data.key2')
  assert_eq!(data['fromBaseA'], 'A', 'merge-contract.data.fromBaseA')
  assert_eq!(data['fromBaseB'], 'B', 'merge-contract.data.fromBaseB')
  assert_eq!(data['ENV_SWITCH'], 'base-production', 'merge-contract.data.ENV_SWITCH')

  merge_contract_dev = find_one!(dev_docs, kind: 'ConfigMap', name: 'merge-contract')
  assert_eq!(merge_contract_dev.dig('data', 'ENV_SWITCH'), 'override-default', 'merge-contract (dev).data.ENV_SWITCH')

  compat_service = find_one!(prod_docs, kind: 'Deployment', name: 'compat-service')
  assert_eq!(compat_service['apiVersion'], 'apps/v1', 'compat-service apiVersion')
  assert_eq!(compat_service.dig('metadata', 'annotations', 'helm-apps/release'), nil, 'compat-service release annotation by default must be absent')
  assert_eq!(compat_service.dig('spec', 'paused'), true, 'compat-service.spec.paused')
  compat_service_main = compat_service.dig('spec', 'template', 'spec', 'containers', 0)
  assert_eq!(compat_service_main.dig('workingDir'), '/app', 'compat-service.main.workingDir')
  compat_checksum = compat_service.dig('spec', 'template', 'metadata', 'annotations', 'checksum/config')
  assert!(compat_checksum.is_a?(String) && !compat_checksum.empty?, 'compat-service checksum/config must be present')
  assert!(compat_checksum != 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'compat-service checksum/config must change when secretConfigFiles content exists')
  compat_service_volume_mount_names = (compat_service_main['volumeMounts'] || []).map { |mount| mount['name'] }
  assert!(compat_service_volume_mount_names.include?('config-containers-compat-service-main-runtime-secret-txt'), 'compat-service secretConfigFiles volumeMount must be present')
  compat_service_volume_names = compat_service.dig('spec', 'template', 'spec', 'volumes').to_a.map { |volume| volume['name'] }
  assert!(compat_service_volume_names.include?('config-containers-compat-service-main-runtime-secret-txt'), 'compat-service secretConfigFiles volume must be present')

  resize_policy = compat_service.dig('spec', 'template', 'spec', 'containers', 0, 'resizePolicy')
  assert!(resize_policy.is_a?(Array) && !resize_policy.empty?, 'compat-service.main.resizePolicy must be present')

  compat_job = find_one!(prod_docs, kind: 'Job', name: 'compat-job')
  assert!(!compat_job.dig('spec', 'podFailurePolicy').nil?, 'compat-job.spec.podFailurePolicy must be present')
  compat_job_vpa = find_one!(prod_docs, kind: 'VerticalPodAutoscaler', name: 'compat-job')
  assert_eq!(compat_job_vpa.dig('spec', 'targetRef', 'kind'), 'Job', 'compat-job VPA targetRef.kind')
  assert_eq!(compat_job_vpa.dig('spec', 'targetRef', 'apiVersion'), 'batch/v1', 'compat-job VPA targetRef.apiVersion')

  compat_daemonset = find_one!(prod_docs, kind: 'DaemonSet', name: 'compat-daemonset')
  assert_eq!(compat_daemonset['apiVersion'], 'apps/v1', 'compat-daemonset apiVersion')
  assert_eq!(compat_daemonset.dig('spec', 'replicas'), nil, 'compat-daemonset.spec.replicas must be absent')
  assert_eq!(compat_daemonset.dig('spec', 'strategy'), nil, 'compat-daemonset.spec.strategy must be absent')
  assert_eq!(compat_daemonset.dig('spec', 'updateStrategy', 'type'), 'RollingUpdate', 'compat-daemonset.spec.updateStrategy.type')
  assert_eq!(compat_daemonset.dig('spec', 'minReadySeconds'), 5, 'compat-daemonset.spec.minReadySeconds')
  assert!(!compat_daemonset.dig('spec', 'selector', 'matchLabels').nil?, 'compat-daemonset.spec.selector.matchLabels must be present')
  compat_daemonset_vpa = find_one!(prod_docs, kind: 'VerticalPodAutoscaler', name: 'compat-daemonset')
  assert_eq!(compat_daemonset_vpa.dig('spec', 'targetRef', 'kind'), 'DaemonSet', 'compat-daemonset VPA targetRef.kind')
  assert_eq!(compat_daemonset_vpa.dig('spec', 'targetRef', 'apiVersion'), 'apps/v1', 'compat-daemonset VPA targetRef.apiVersion')
  find_one!(prod_docs, kind: 'Service', name: 'compat-daemonset')

  compat_cron_vpa = find_one!(prod_docs, kind: 'VerticalPodAutoscaler', name: 'compat-cron')
  assert_eq!(compat_cron_vpa.dig('spec', 'targetRef', 'kind'), 'CronJob', 'compat-cron VPA targetRef.kind')
  compat_cron_target_api = compat_cron_vpa.dig('spec', 'targetRef', 'apiVersion')
  assert!(%w[batch/v1 batch/v1beta1].include?(compat_cron_target_api), "compat-cron VPA targetRef.apiVersion: expected batch/v1 or batch/v1beta1, got #{compat_cron_target_api.inspect}")

  compat_ingress = find_one!(prod_docs, kind: 'Ingress', name: 'compat-ingress')
  assert_eq!(compat_ingress.dig('spec', 'defaultBackend', 'service', 'name'), 'compat-service', 'compat-ingress.spec.defaultBackend.service.name')

  compat_pvc = find_one!(prod_docs, kind: 'PersistentVolumeClaim', name: 'compat-pvc')
  assert_eq!(compat_pvc.dig('spec', 'volumeMode'), 'Filesystem', 'compat-pvc.spec.volumeMode')

  compat_config = find_one!(prod_docs, kind: 'ConfigMap', name: 'compat-config')
  assert_eq!(compat_config.dig('immutable'), true, 'compat-config.immutable')

  compat_secret = find_one!(prod_docs, kind: 'Secret', name: 'compat-secret')
  assert_eq!(compat_secret.dig('stringData', 'token'), 'value', 'compat-secret.stringData.token')

  common_runtime_secret = find_one!(prod_docs, kind: 'Secret', name: 'common-runtime')
  assert_eq!(common_runtime_secret.dig('data', 'SHARED_MODE'), 'c3RyaWN0', 'common-runtime.data.SHARED_MODE')
  assert_eq!(common_runtime_secret.dig('data', 'SHARED_REGION'), 'ZXUtY2VudHJhbC0x', 'common-runtime.data.SHARED_REGION')

  netpol_k8s = find_one!(prod_docs, kind: 'NetworkPolicy', name: 'compat-netpol', api_version: 'networking.k8s.io/v1')
  assert_eq!(netpol_k8s.dig('spec', 'ingress', 0, 'from', 0, 'namespaceSelector', 'matchLabels', 'kubernetes.io/metadata.name'), 'ingress-nginx', 'compat-netpol ingress namespace selector')
  assert_eq!(netpol_k8s.dig('spec', 'egress', 0, 'ports', 0, 'port'), 53, 'compat-netpol egress DNS port')

  cilium_netpol = find_one!(prod_docs, kind: 'CiliumNetworkPolicy', name: 'compat-cilium-netpol', api_version: 'cilium.io/v2')
  assert_eq!(cilium_netpol.dig('spec', 'endpointSelector', 'matchLabels', 'app'), 'compat-service', 'compat-cilium-netpol selector app')

  calico_netpol = find_one!(prod_docs, kind: 'NetworkPolicy', name: 'compat-calico-netpol', api_version: 'projectcalico.org/v3')
  assert_eq!(calico_netpol.dig('spec', 'selector'), "app == 'compat-service'", 'compat-calico-netpol.spec.selector')

  release_auto_app = find_one!(prod_docs, kind: 'Deployment', name: 'release-auto-app')
  assert_eq!(release_auto_app.dig('spec', 'template', 'spec', 'containers', 0, 'image'), 'alpine:3.19', 'release-auto-app image')
  assert_eq!(release_auto_app.dig('metadata', 'annotations', 'helm-apps/release'), 'production-v1', 'release-auto-app release annotation')
  assert_eq!(release_auto_app.dig('metadata', 'annotations', 'helm-apps/app-version'), '3.19', 'release-auto-app app-version annotation')

  compat_route = find_one!(prod_docs, kind: 'Ingress', name: 'compat-route')
  assert_eq!(compat_route.dig('spec', 'rules', 0, 'host'), 'route.example.com', 'compat-route host')

  # EnvFrom order contracts (shared env + manual envFrom + secretEnvVars auto-secret).
  compat_service_env_from = env_from_refs(compat_service)
  assert_eq!(compat_service_env_from, ['configMapRef:common-runtime-cm', 'secretRef:common-runtime'], 'compat-service envFrom order')

  compat_env_old = find_one!(prod_docs, kind: 'Deployment', name: 'compat-env-old')
  assert_eq!(env_from_refs(compat_env_old), ['secretRef:manual-env-old', 'secretRef:envs-containers-compat-env-old-main'], 'compat-env-old envFrom order')

  compat_env_mixed = find_one!(prod_docs, kind: 'Deployment', name: 'compat-env-mixed')
  assert_eq!(env_from_refs(compat_env_mixed), ['secretRef:common-runtime', 'secretRef:manual-env-mixed', 'secretRef:envs-containers-compat-env-mixed-main'], 'compat-env-mixed envFrom order')
  compat_env_mixed_prod_env = compat_env_mixed.dig('spec', 'template', 'spec', 'containers', 0, 'env')
  prod_exact = compat_env_mixed_prod_env.find { |entry| entry['name'] == 'FROM_SECRET_EXACT' }
  prod_regex = compat_env_mixed_prod_env.find { |entry| entry['name'] == 'FROM_SECRET_REGEX' }
  assert_eq!(prod_exact.dig('valueFrom', 'secretKeyRef', 'key'), 'exact-production-key', 'fromSecretsEnvVars exact env selection')
  assert_eq!(prod_regex.dig('valueFrom', 'secretKeyRef', 'key'), 'regex-default-key', 'fromSecretsEnvVars default selection')

  compat_env_mixed_dev = find_one!(dev_docs, kind: 'Deployment', name: 'compat-env-mixed')
  compat_env_mixed_dev_env = compat_env_mixed_dev.dig('spec', 'template', 'spec', 'containers', 0, 'env')
  dev_exact = compat_env_mixed_dev_env.find { |entry| entry['name'] == 'FROM_SECRET_EXACT' }
  dev_regex = compat_env_mixed_dev_env.find { |entry| entry['name'] == 'FROM_SECRET_REGEX' }
  assert_eq!(dev_exact.dig('valueFrom', 'secretKeyRef', 'key'), 'exact-default-key', 'fromSecretsEnvVars dev default selection')
  assert_eq!(dev_regex.dig('valueFrom', 'secretKeyRef', 'key'), 'regex-dev-key', 'fromSecretsEnvVars regex selection')

  compat_env_list_miss_prod = find_one!(prod_docs, kind: 'Deployment', name: 'compat-env-list-miss')
  assert_eq!(compat_env_list_miss_prod.dig('spec', 'template', 'spec', 'imagePullSecrets'), nil, 'compat-env-list-miss(prod) imagePullSecrets must be absent')
  assert_eq!(compat_env_list_miss_prod.dig('spec', 'template', 'spec', 'containers', 0, 'ports'), nil, 'compat-env-list-miss(prod) container ports must be absent')
  assert!(find_docs(prod_docs, kind: 'Service', name: 'compat-env-list-miss').empty?, 'compat-env-list-miss(prod) service must be absent when ports are unresolved')

  compat_env_list_miss_dev = find_one!(dev_docs, kind: 'Deployment', name: 'compat-env-list-miss')
  assert_eq!(compat_env_list_miss_dev.dig('spec', 'template', 'spec', 'imagePullSecrets', 0, 'name'), 'compat-regcred', 'compat-env-list-miss(dev) imagePullSecrets[0].name')
  assert_eq!(compat_env_list_miss_dev.dig('spec', 'template', 'spec', 'containers', 0, 'ports', 0, 'containerPort'), 8091, 'compat-env-list-miss(dev) container port')
  compat_env_list_miss_service_dev = find_one!(dev_docs, kind: 'Service', name: 'compat-env-list-miss')
  assert_eq!(compat_env_list_miss_service_dev.dig('spec', 'ports', 0, 'port'), 8091, 'compat-env-list-miss(dev) service port')

  compat_generic_envlike_dev = find_one!(dev_docs, kind: 'WidgetPolicy', name: 'compat-generic-envlike-dev')
  compat_generic_envlike_dev_literal = compat_generic_envlike_dev.dig('spec', 'literal')
  assert_eq!(compat_generic_envlike_dev_literal, 'keepme', 'compat-generic-envlike-dev(dev) spec.literal')

  compat_generic_envlike_default = find_one!(prod_docs, kind: 'WidgetPolicy', name: 'compat-generic-envlike-default')
  compat_generic_envlike_default_literal = compat_generic_envlike_default.dig('spec', 'literal')
  assert_eq!(compat_generic_envlike_default_literal, 'keepme', 'compat-generic-envlike-default(prod) spec.literal')

  compat_native_list_envlike_dev = find_one!(dev_docs, kind: 'Deployment', name: 'compat-native-list-envlike-dev')
  compat_native_list_envlike_dev_secret = compat_native_list_envlike_dev.dig('spec', 'template', 'spec', 'imagePullSecrets', 0)
  assert!(compat_native_list_envlike_dev_secret.is_a?(Hash), 'compat-native-list-envlike-dev(dev) imagePullSecrets[0] must stay map')
  assert_eq!(compat_native_list_envlike_dev_secret['name'], 'compat-regcred', 'compat-native-list-envlike-dev(dev) imagePullSecrets[0].name')
  assert_eq!(compat_native_list_envlike_dev_secret['dev'], 'keepme', 'compat-native-list-envlike-dev(dev) imagePullSecrets[0].dev')
  assert_eq!(compat_native_list_envlike_dev_secret['other'], 'stay', 'compat-native-list-envlike-dev(dev) imagePullSecrets[0].other')

  compat_native_list_envlike_default = find_one!(prod_docs, kind: 'Deployment', name: 'compat-native-list-envlike-default')
  compat_native_list_envlike_default_secret = compat_native_list_envlike_default.dig('spec', 'template', 'spec', 'imagePullSecrets', 0)
  assert!(compat_native_list_envlike_default_secret.is_a?(Hash), 'compat-native-list-envlike-default(prod) imagePullSecrets[0] must stay map')
  assert_eq!(compat_native_list_envlike_default_secret['name'], 'compat-regcred', 'compat-native-list-envlike-default(prod) imagePullSecrets[0].name')
  assert_eq!(compat_native_list_envlike_default_secret['_default'], 'keepme', 'compat-native-list-envlike-default(prod) imagePullSecrets[0]._default')
  assert_eq!(compat_native_list_envlike_default_secret['another'], 'stay', 'compat-native-list-envlike-default(prod) imagePullSecrets[0].another')

  compat_native_list_literal_tpl = find_one!(prod_docs, kind: 'Deployment', name: 'compat-native-list-literal-tpl')
  assert_eq!(compat_native_list_literal_tpl.dig('spec', 'template', 'spec', 'imagePullSecrets', 0, 'name'), '{{ $.Values.jwtSigningMethod }}', 'compat-native-list-literal-tpl(prod) imagePullSecrets[0].name must stay literal')

  compat_string_list_tpl = find_one!(prod_docs, kind: 'Deployment', name: 'compat-string-list-tpl')
  assert_eq!(compat_string_list_tpl.dig('spec', 'template', 'spec', 'imagePullSecrets', 0, 'name'), 'rsa', 'compat-string-list-tpl(prod) imagePullSecrets[0].name')

  compat_generic_string_list_tpl = find_one!(prod_docs, kind: 'WidgetPolicy', name: 'compat-generic-string-list-tpl')
  assert_eq!(compat_generic_string_list_tpl.dig('spec', 'rules', 0, 'action'), 'rsa', 'compat-generic-string-list-tpl(prod) spec.rules[0].action')

  verify_required_entities!(prod_docs)

  strict_custom = find_one!(strict_docs, kind: 'ConfigMap', name: 'custom-group-cm')
  assert_eq!(strict_custom.dig('data', 'custom'), 'ok', 'strict mode custom-group-cm.data.custom')

  service_129 = find_one!(k129_docs, kind: 'Service', name: 'compat-service')
  assert_eq!(service_129.dig('spec', 'loadBalancerClass'), 'internal-vip', 'k8s 1.29 loadBalancerClass')
  assert_eq!(service_129.dig('spec', 'internalTrafficPolicy'), 'Local', 'k8s 1.29 internalTrafficPolicy')

  service_120 = find_one!(k120_docs, kind: 'Service', name: 'compat-service')
  assert_eq!(service_120.dig('spec', 'loadBalancerClass'), nil, 'k8s 1.20 loadBalancerClass must be absent')
  assert_eq!(service_120.dig('spec', 'internalTrafficPolicy'), nil, 'k8s 1.20 internalTrafficPolicy must be absent')
  assert_eq!(service_120.dig('spec', 'ipFamilyPolicy'), 'SingleStack', 'k8s 1.20 ipFamilyPolicy')
  assert_eq!(service_120.dig('spec', 'allocateLoadBalancerNodePorts'), true, 'k8s 1.20 allocateLoadBalancerNodePorts')
  cron_vpa_120 = find_one!(k120_docs, kind: 'VerticalPodAutoscaler', name: 'compat-cron')
  assert_eq!(cron_vpa_120.dig('spec', 'targetRef', 'apiVersion'), 'batch/v1beta1', 'k8s 1.20 compat-cron VPA targetRef.apiVersion')
  job_vpa_120 = find_one!(k120_docs, kind: 'VerticalPodAutoscaler', name: 'compat-job')
  assert_eq!(job_vpa_120.dig('spec', 'targetRef', 'apiVersion'), 'batch/v1', 'k8s 1.20 compat-job VPA targetRef.apiVersion')

  service_119 = find_one!(k119_docs, kind: 'Service', name: 'compat-service')
  assert_eq!(service_119.dig('spec', 'loadBalancerClass'), nil, 'k8s 1.19 loadBalancerClass must be absent')
  assert_eq!(service_119.dig('spec', 'internalTrafficPolicy'), nil, 'k8s 1.19 internalTrafficPolicy must be absent')
  assert_eq!(service_119.dig('spec', 'ipFamilyPolicy'), nil, 'k8s 1.19 ipFamilyPolicy must be absent')
  assert_eq!(service_119.dig('spec', 'ipFamilies'), nil, 'k8s 1.19 ipFamilies must be absent')
  assert_eq!(service_119.dig('spec', 'allocateLoadBalancerNodePorts'), nil, 'k8s 1.19 allocateLoadBalancerNodePorts must be absent')
  cron_vpa_119 = find_one!(k119_docs, kind: 'VerticalPodAutoscaler', name: 'compat-cron')
  assert_eq!(cron_vpa_119.dig('spec', 'targetRef', 'apiVersion'), 'batch/v1beta1', 'k8s 1.19 compat-cron VPA targetRef.apiVersion')
  job_vpa_119 = find_one!(k119_docs, kind: 'VerticalPodAutoscaler', name: 'compat-job')
  assert_eq!(job_vpa_119.dig('spec', 'targetRef', 'apiVersion'), 'batch/v1', 'k8s 1.19 compat-job VPA targetRef.apiVersion')

  verify_version_gates!(kmodern_docs, k129_docs, k120_docs)
end

def verify_internal!(path)
  docs = load_docs(path)

  compat_web = find_one!(docs, kind: 'Deployment', name: 'compat-web')
  assert_eq!(compat_web.dig('spec', 'template', 'spec', 'containers', 0, 'image'), 'alpine:1.2.3', 'compat-web image')
  assert_eq!(compat_web.dig('metadata', 'annotations', 'helm-apps/release'), 'r1', 'compat-web release annotation')
  assert_eq!(compat_web.dig('metadata', 'annotations', 'helm-apps/app-version'), '1.2.3', 'compat-web app-version annotation')

  compat_route = find_one!(docs, kind: 'Ingress', name: 'compat-route')
  assert_eq!(compat_route.dig('spec', 'rules', 0, 'host'), 'compat.example.com', 'internal compat-route host')
end

def parse_main_args(argv)
  options = {}
  parser = OptionParser.new do |opts|
    opts.banner = 'Usage: scripts/verify-contracts-structure.rb main --production FILE --dev FILE --strict FILE --k129 FILE --k120 FILE --k119 FILE --kmodern FILE'
    opts.on('--production FILE', String) { |v| options[:production] = v }
    opts.on('--dev FILE', String) { |v| options[:dev] = v }
    opts.on('--strict FILE', String) { |v| options[:strict] = v }
    opts.on('--k129 FILE', String) { |v| options[:k129] = v }
    opts.on('--k120 FILE', String) { |v| options[:k120] = v }
    opts.on('--k119 FILE', String) { |v| options[:k119] = v }
    opts.on('--kmodern FILE', String) { |v| options[:kmodern] = v }
  end
  parser.parse!(argv)

  required = %i[production dev strict k129 k120 k119 kmodern]
  missing = required.reject { |key| options.key?(key) }
  assert!(missing.empty?, "Missing required args for main mode: #{missing.join(', ')}")

  options
end

def parse_internal_args(argv)
  options = {}
  parser = OptionParser.new do |opts|
    opts.banner = 'Usage: scripts/verify-contracts-structure.rb internal --file FILE'
    opts.on('--file FILE', String) { |v| options[:file] = v }
  end
  parser.parse!(argv)

  assert!(options.key?(:file), 'Missing required arg for internal mode: file')

  options
end

begin
  mode = ARGV.shift

  case mode
  when 'main'
    verify_main!(parse_main_args(ARGV))
    puts 'Contract structure checks passed (main).'
  when 'internal'
    verify_internal!(parse_internal_args(ARGV)[:file])
    puts 'Contract structure checks passed (internal).'
  else
    warn 'Usage:'
    warn '  scripts/verify-contracts-structure.rb main --production FILE --dev FILE --strict FILE --k129 FILE --k120 FILE --k119 FILE --kmodern FILE'
    warn '  scripts/verify-contracts-structure.rb internal --file FILE'
    exit 2
  end
rescue AssertionError => e
  warn "Contract structure verification failed: #{e.message}"
  exit 1
end
