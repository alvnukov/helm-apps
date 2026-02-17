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
    ['StatefulSet', 'compat-stateful'],
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

def verify_main!(paths)
  prod_docs = load_docs(paths[:production])
  dev_docs = load_docs(paths[:dev])
  strict_docs = load_docs(paths[:strict])
  k129_docs = load_docs(paths[:k129])
  k120_docs = load_docs(paths[:k120])
  k119_docs = load_docs(paths[:k119])

  merge_contract = find_one!(prod_docs, kind: 'ConfigMap', name: 'merge-contract')
  data = merge_contract['data'] || {}
  assert_eq!(data['A'], '2', 'merge-contract.data.A')
  assert_eq!(data['LOCAL'], 'ok', 'merge-contract.data.LOCAL')
  assert_eq!(data['key1'], 'value-1', 'merge-contract.data.key1')
  assert_eq!(data['key2'], 'local-value-2', 'merge-contract.data.key2')
  assert_eq!(data['fromBaseA'], 'A', 'merge-contract.data.fromBaseA')
  assert_eq!(data['fromBaseB'], 'B', 'merge-contract.data.fromBaseB')
  assert_eq!(data['ENV_SWITCH'], 'override-default', 'merge-contract.data.ENV_SWITCH')

  merge_contract_dev = find_one!(dev_docs, kind: 'ConfigMap', name: 'merge-contract')
  assert_eq!(merge_contract_dev.dig('data', 'ENV_SWITCH'), 'override-default', 'merge-contract (dev).data.ENV_SWITCH')

  compat_service = find_one!(prod_docs, kind: 'Deployment', name: 'compat-service')
  assert_eq!(compat_service['apiVersion'], 'apps/v1', 'compat-service apiVersion')
  assert_eq!(compat_service.dig('spec', 'paused'), true, 'compat-service.spec.paused')

  resize_policy = compat_service.dig('spec', 'template', 'spec', 'containers', 0, 'resizePolicy')
  assert!(resize_policy.is_a?(Array) && !resize_policy.empty?, 'compat-service.main.resizePolicy must be present')

  compat_job = find_one!(prod_docs, kind: 'Job', name: 'compat-job')
  assert!(!compat_job.dig('spec', 'podFailurePolicy').nil?, 'compat-job.spec.podFailurePolicy must be present')

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

  service_119 = find_one!(k119_docs, kind: 'Service', name: 'compat-service')
  assert_eq!(service_119.dig('spec', 'loadBalancerClass'), nil, 'k8s 1.19 loadBalancerClass must be absent')
  assert_eq!(service_119.dig('spec', 'internalTrafficPolicy'), nil, 'k8s 1.19 internalTrafficPolicy must be absent')
  assert_eq!(service_119.dig('spec', 'ipFamilyPolicy'), nil, 'k8s 1.19 ipFamilyPolicy must be absent')
  assert_eq!(service_119.dig('spec', 'ipFamilies'), nil, 'k8s 1.19 ipFamilies must be absent')
  assert_eq!(service_119.dig('spec', 'allocateLoadBalancerNodePorts'), nil, 'k8s 1.19 allocateLoadBalancerNodePorts must be absent')
end

def verify_internal!(path)
  docs = load_docs(path)

  compat_web = find_one!(docs, kind: 'Deployment', name: 'compat-web')
  assert_eq!(compat_web.dig('spec', 'template', 'spec', 'containers', 0, 'image'), 'alpine:1.2.3', 'compat-web image')
  assert_eq!(compat_web.dig('metadata', 'annotations', 'helm-apps/release'), 'production-v1', 'compat-web release annotation')
  assert_eq!(compat_web.dig('metadata', 'annotations', 'helm-apps/app-version'), '1.2.3', 'compat-web app-version annotation')

  compat_route = find_one!(docs, kind: 'Ingress', name: 'compat-route')
  assert_eq!(compat_route.dig('spec', 'rules', 0, 'host'), 'compat.example.com', 'internal compat-route host')
end

def parse_main_args(argv)
  options = {}
  parser = OptionParser.new do |opts|
    opts.banner = 'Usage: scripts/verify-contracts-structure.rb main --production FILE --dev FILE --strict FILE --k129 FILE --k120 FILE --k119 FILE'
    opts.on('--production FILE', String) { |v| options[:production] = v }
    opts.on('--dev FILE', String) { |v| options[:dev] = v }
    opts.on('--strict FILE', String) { |v| options[:strict] = v }
    opts.on('--k129 FILE', String) { |v| options[:k129] = v }
    opts.on('--k120 FILE', String) { |v| options[:k120] = v }
    opts.on('--k119 FILE', String) { |v| options[:k119] = v }
  end
  parser.parse!(argv)

  required = %i[production dev strict k129 k120 k119]
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
    warn '  scripts/verify-contracts-structure.rb main --production FILE --dev FILE --strict FILE --k129 FILE --k120 FILE --k119 FILE'
    warn '  scripts/verify-contracts-structure.rb internal --file FILE'
    exit 2
  end
rescue AssertionError => e
  warn "Contract structure verification failed: #{e.message}"
  exit 1
end
