#!/usr/bin/env ruby
# frozen_string_literal: true

# Verifies the Kubernetes API compatibility gates of the helm-apps library
# against a single rendered manifest stream.
#
# Every entry in GATES names a field the library emits only at or above the
# Kubernetes release that introduced it in the API schema. Given the version the
# stream was rendered for, the field must be present at or above that release
# and absent below it. The gate versions are verified against the per-version
# Kubernetes JSON schemas; see docs/operations.md#kubernetes-api-compatibility.
#
# Usage: scripts/verify-kube-gates.rb --file FILE --kube-version 1.29.0

require 'optparse'
require 'yaml'

class GateError < StandardError; end

# [kind, name, path, min_version, description]
GATES = [
  # Service fields that moved into the core API over 1.20-1.30.
  ['Service', 'compat-service', %w[spec allocateLoadBalancerNodePorts], '1.20'],
  ['Service', 'compat-service', %w[spec ipFamilies], '1.20'],
  ['Service', 'compat-service', %w[spec ipFamilyPolicy], '1.20'],
  ['Service', 'compat-service', %w[spec internalTrafficPolicy], '1.21'],
  ['Service', 'compat-service', %w[spec loadBalancerClass], '1.21'],
  ['Service', 'compat-modern-apis', %w[spec trafficDistribution], '1.30'],

  # StatefulSetSpec.
  ['StatefulSet', 'compat-stateful', %w[spec minReadySeconds], '1.22'],
  ['StatefulSet', 'compat-stateful', %w[spec persistentVolumeClaimRetentionPolicy], '1.23'],
  ['StatefulSet', 'compat-stateful', %w[spec ordinals], '1.26'],

  # PodDisruptionBudgetSpec.
  ['PodDisruptionBudget', 'compat-modern-apis', %w[spec unhealthyPodEvictionPolicy], '1.26'],

  # CronJobSpec.
  ['CronJob', 'compat-cron', %w[spec timeZone], '1.24'],

  # JobSpec.
  ['Job', 'compat-job-indexed', %w[spec completionMode], '1.21'],
  ['Job', 'compat-job-indexed', %w[spec suspend], '1.21'],
  ['Job', 'compat-job-indexed', %w[spec podFailurePolicy], '1.25'],
  ['Job', 'compat-job-indexed', %w[spec backoffLimitPerIndex], '1.28'],
  ['Job', 'compat-job-indexed', %w[spec maxFailedIndexes], '1.28'],
  ['Job', 'compat-job-indexed', %w[spec podReplacementPolicy], '1.28'],
  ['Job', 'compat-job-indexed', %w[spec managedBy], '1.30'],
  ['Job', 'compat-job-indexed', %w[spec successPolicy], '1.30'],

  # PodSpec.
  ['Deployment', 'compat-modern-apis', ['spec', 'template', 'spec', 'hostUsers'], '1.25'],
  ['Deployment', 'compat-modern-apis', ['spec', 'template', 'spec', 'schedulingGates'], '1.26'],

  # ContainerSpec.
  ['Deployment', 'compat-modern-apis', ['spec', 'template', 'spec', 'containers', 0, 'resizePolicy'], '1.27'],
  ['Deployment', 'compat-modern-apis', ['spec', 'template', 'spec', 'initContainers', 0, 'restartPolicy'], '1.28']
].freeze

# [kind, name, min_version, stable_api_version, legacy_api_version]
API_VERSION_GATES = [
  ['CronJob', 'compat-cron', '1.21', 'batch/v1', 'batch/v1beta1'],
  ['PodDisruptionBudget', 'compat-modern-apis', '1.21', 'policy/v1', 'policy/v1beta1']
].freeze

# Fields the library must never emit, whatever the cluster version.
FORBIDDEN = [
  ['StatefulSet', 'compat-stateful', %w[spec progressDeadlineSeconds]],
  ['DaemonSet', 'compat-daemonset', %w[spec replicas]],
  ['DaemonSet', 'compat-daemonset', %w[spec strategy]]
].freeze

def minor(version)
  parts = version.to_s.split('.')
  [parts[0].to_i, parts[1].to_i]
end

def at_least?(version, minimum)
  (minor(version) <=> minor(minimum)) >= 0
end

def find_one!(docs, kind, name)
  doc = docs.find { |d| d['kind'] == kind && d.dig('metadata', 'name') == name }
  raise GateError, "missing #{kind}/#{name} in render" if doc.nil?

  doc
end

def dig_path(doc, path)
  path.reduce(doc) do |scope, key|
    return nil if scope.nil?
    return nil if key.is_a?(Integer) && !scope.is_a?(Array)
    return nil if key.is_a?(String) && !scope.is_a?(Hash)

    scope[key]
  end
end

def verify!(path, kube_version)
  docs = YAML.load_stream(File.read(path)).compact.select { |doc| doc.is_a?(Hash) }
  failures = []

  GATES.each do |kind, name, field_path, min_version|
    doc = find_one!(docs, kind, name)
    value = dig_path(doc, field_path)
    expected = at_least?(kube_version, min_version)
    pretty = "#{kind}/#{name}.#{field_path.join('.')}"

    if expected && value.nil?
      failures << "#{pretty} must be present on Kubernetes #{kube_version} (gate #{min_version})"
    elsif !expected && !value.nil?
      failures << "#{pretty} must be absent on Kubernetes #{kube_version} (gate #{min_version}), got #{value.inspect}"
    end
  end

  API_VERSION_GATES.each do |kind, name, min_version, stable, legacy|
    doc = find_one!(docs, kind, name)
    expected = at_least?(kube_version, min_version) ? stable : legacy
    next if doc['apiVersion'] == expected

    failures << "#{kind}/#{name} apiVersion on Kubernetes #{kube_version}: expected #{expected}, got #{doc['apiVersion'].inspect}"
  end

  FORBIDDEN.each do |kind, name, field_path|
    doc = find_one!(docs, kind, name)
    value = dig_path(doc, field_path)
    next if value.nil?

    failures << "#{kind}/#{name}.#{field_path.join('.')} must never be emitted, got #{value.inspect}"
  end

  raise GateError, failures.join("\n  ") unless failures.empty?
end

options = {}
OptionParser.new do |opts|
  opts.banner = 'Usage: scripts/verify-kube-gates.rb --file FILE --kube-version VERSION'
  opts.on('--file FILE', String) { |v| options[:file] = v }
  opts.on('--kube-version VERSION', String) { |v| options[:kube_version] = v }
end.parse!(ARGV)

if options[:file].nil? || options[:kube_version].nil?
  warn 'Usage: scripts/verify-kube-gates.rb --file FILE --kube-version VERSION'
  exit 2
end

begin
  verify!(options[:file], options[:kube_version])
  puts "Kubernetes API gates OK for #{options[:kube_version]} (#{options[:file]})."
rescue GateError => e
  warn "Kubernetes API gate verification failed for #{options[:kube_version]}:"
  warn "  #{e.message}"
  exit 1
end
