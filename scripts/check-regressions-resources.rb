#!/usr/bin/env ruby
# frozen_string_literal: true

require 'fileutils'
require 'open3'
require 'psych'
require 'tmpdir'

ROOT = File.expand_path('..', __dir__)

def assert(condition, message)
  raise message unless condition
end

def resource(documents, kind, name = nil)
  documents.find { |d| d['kind'] == kind && (!name || d.dig('metadata', 'name') == name) } ||
    raise("missing #{kind}/#{name}")
end

def render(chart, values, expected_error: nil)
  file = File.join(File.dirname(chart), 'case.yaml')
  File.write(file, Psych.dump(values))
  out, err, status = Open3.capture3('helm', 'template', 'resources', chart, '-f', file,
                                   '--namespace', 'release-space', '--kube-version', '1.29.0')
  if expected_error
    assert(!status.success? && err.include?(expected_error), "expected #{expected_error}, got #{err}")
    return
  end
  assert(status.success?, "helm render failed: #{err}")
  yaml_file = File.join(File.dirname(chart), 'rendered.yaml')
  File.write(yaml_file, out)
  validation, validation_error, validated = Open3.capture3('ruby', File.join(ROOT, 'scripts/validate-yaml-stream.rb'), yaml_file)
  assert(validated.success?, "invalid YAML stream: #{validation}#{validation_error}")
  Psych.load_stream(out).compact
end

def ingress
  {'enabled' => true, 'ingressClassName' => 'nginx', 'host' => 'web.example.com',
   'paths' => "- path: /\n  pathType: Prefix\n  backend:\n    service:\n      name: web\n      port:\n        number: 80\n",
   'tls' => {'enabled' => true, 'secret_name' => 'existing-tls'}}
end

def service_account
  {'enabled' => true, 'namespace' => 'apps', 'roles' => {
    'reader' => {'namespace' => 'shared', 'rules' => {
      'read' => {'apiGroups' => [''], 'resources' => ['configmaps'], 'verbs' => ['get'],
                 'resourceNames' => ['runtime']}}}}}
end

cases = {}
cases['dex'] = lambda do |chart|
  web = ingress.merge('dexAuth' => {'enabled' => true, 'clusterDomain' => 'cluster.local'})
  docs = render(chart, {'apps-ingresses' => {'web' => web}})
  annotations = resource(docs, 'Ingress').dig('metadata', 'annotations') || {}
  assert(annotations['nginx.ingress.kubernetes.io/auth-url'] ==
    'https://web-dex-authenticator.release-space.svc.cluster.local/dex-authenticator/auth', 'Dex auth-url missing or wrong namespace')
  assert(annotations['nginx.ingress.kubernetes.io/auth-signin'] == 'https://$host/dex-authenticator/sign_in', 'Dex auth-signin missing')
  resource(docs, 'DexAuthenticator')
  ['werf-space', {'_default' => 'werf-space'}].each do |namespace|
    docs = render(chart, {'werf' => {'namespace' => namespace}, 'apps-ingresses' => {'web' => web}})
    assert(resource(docs, 'Ingress').dig('metadata', 'annotations', 'nginx.ingress.kubernetes.io/auth-url') ==
      'https://web-dex-authenticator.werf-space.svc.cluster.local/dex-authenticator/auth', 'explicit werf namespace ignored')
  end
  docs = render(chart, {'werf' => {'namespace' => ''}, 'apps-ingresses' => {'web' => web}})
  assert(resource(docs, 'Ingress').dig('metadata', 'annotations', 'nginx.ingress.kubernetes.io/auth-url') ==
    'https://web-dex-authenticator.release-space.svc.cluster.local/dex-authenticator/auth', 'empty werf namespace did not fall back to release namespace')
  docs = render(chart, {'apps-ingresses' => {'web' => ingress}})
  assert(!(resource(docs, 'Ingress').dig('metadata', 'annotations') || {}).key?('nginx.ingress.kubernetes.io/auth-url'), 'disabled Dex generated auth annotations')
end
cases['network-spec'] = lambda do |chart|
  map = render(chart, {'apps-network-policies' => {'deny-all' => {'enabled' => true, 'spec' => {'podSelector' => {}}}}})
  block = render(chart, {'apps-network-policies' => {'deny-all' => {'enabled' => true, 'spec' => "podSelector: {}\n"}}})
  assert(resource(map, 'NetworkPolicy')['spec'] == {'podSelector' => {}}, 'native spec replaced by generated selector')
  assert(resource(map, 'NetworkPolicy')['spec'] == resource(block, 'NetworkPolicy')['spec'], 'map and block spec differ')
  spec = {'podSelector' => {'matchLabels' => {'prod' => 'yes', 'app' => 'api'}}}
  docs = render(chart, {'apps-network-policies' => {'policy' => {'enabled' => true, 'spec' => spec}}})
  assert(resource(docs, 'NetworkPolicy')['spec'] == spec, 'native spec data interpreted as nested env-map')
  docs = render(chart, {'apps-network-policies' => {'policy' => {'enabled' => true, 'spec' => {'_default' => spec}}}})
  assert(resource(docs, 'NetworkPolicy')['spec'] == spec, 'root spec env selection failed')
end
cases['network-release'] = lambda do |chart|
  values = {'global' => {'env' => 'prod', 'validation' => {'strict' => true},
    'deploy' => {'enabled' => true, 'release' => 'v1'}, 'releases' => {'v1' => {'policy' => '1.0.0'}}},
    'apps-network-policies' => {'policy' => {'enabled' => true, 'podSelector' => 'matchLabels: {app: api}'}}}
  docs = render(chart, values)
  assert(resource(docs, 'NetworkPolicy').dig('metadata', 'annotations', 'helm-apps/app-version') == '1.0.0', 'release version annotation missing')
  values['apps-network-policies']['policy']['versionKey'] = 'policy'
  render(chart, values)
  values['apps-network-policies']['policy']['unsupported'] = true
  render(chart, values, expected_error: 'E_STRICT_UNKNOWN_KEY')
end
cases['role-namespace'] = lambda do |chart|
  sa = service_account
  docs = render(chart, {'apps-service-accounts' => {'app' => sa}})
  assert(resource(docs, 'RoleBinding').dig('metadata', 'namespace') == resource(docs, 'Role').dig('metadata', 'namespace'), 'RoleBinding namespace does not match Role')
  assert(resource(docs, 'RoleBinding').dig('subjects', 0, 'namespace') == 'apps', 'subject namespace changed')
  sa['roles']['reader']['binding'] = {'namespace' => 'explicit'}
  docs = render(chart, {'apps-service-accounts' => {'app' => sa}})
  assert(resource(docs, 'RoleBinding').dig('metadata', 'namespace') == 'explicit', 'explicit binding namespace lost')
end
cases['rbac-lists'] = lambda do |chart|
  sa = service_account
  sa['roles']['reader.dot'] = sa['roles'].delete('reader')
  sa['roles']['reader.dot']['rules']['read.dot'] = sa['roles']['reader.dot']['rules'].delete('read')
  sa['roles']['reader.dot']['binding'] = {'subjects' => [{'kind' => 'ServiceAccount', 'name' => 'my.sa', 'namespace' => 'apps'}]}
  sa['clusterRoles'] = {'metrics.dot' => {'rules' => {'read.url' => {'nonResourceURLs' => ['/metrics'], 'verbs' => ['get']}},
    'binding' => {'subjects' => [{'kind' => 'ServiceAccount', 'name' => 'my.sa', 'namespace' => 'apps'}]}}}
  parent = {'enabled' => true, 'containers' => {'main' => {'image' => {'name' => 'nginx', 'staticTag' => '1.27'}}},
    'childApps' => {'apps-service-accounts' => {'my.sa' => sa}}}
  inputs = [
    {'apps-service-accounts' => {'my.sa' => sa}},
    {'custom-sa' => {'__GroupVars__' => {'type' => {'_default' => 'apps-service-accounts'}}, 'my.sa' => sa}},
    {'custom-sa' => {'__GroupVars__' => {'type' => 'apps-configmaps'}, 'my.sa' => sa.merge('__AppType__' => 'apps-service-accounts')}},
    {'custom-sa' => {'__GroupVars__' => {'type' => 'apps-service-accounts'},
      'nested.group' => {'__GroupVars__' => {}, 'my.sa' => sa}}},
    {'apps-stateless' => {'parent' => parent}},
    {'custom-workloads' => {'__GroupVars__' => {'type' => {'_default' => 'apps-stateless'}}, 'parent' => parent}}
  ]
  inputs.each do |values|
    docs = render(chart, values)
    assert(resource(docs, 'Role')['rules'][0]['verbs'] == ['get'], 'native RBAC verbs lost')
    assert(resource(docs, 'RoleBinding')['subjects'][0]['name'] == 'my.sa', 'native subjects lost')
    assert(resource(docs, 'ClusterRole')['rules'][0]['nonResourceURLs'] == ['/metrics'], 'native cluster RBAC URLs lost')
    assert(resource(docs, 'ClusterRoleBinding')['subjects'][0]['name'] == 'my.sa', 'native cluster subjects lost')
    assert(!resource(docs, 'ClusterRole').fetch('metadata').key?('namespace'), 'ClusterRole acquired namespace')
  end
  render(chart, {'apps-stateless' => {'spoof' => sa}}, expected_error: 'E_UNEXPECTED_LIST')
  render(chart, {'apps-service-accounts' => {'spoof' => sa.merge('__AppType__' => 'apps-configmaps')}}, expected_error: 'E_UNEXPECTED_LIST')
  render(chart, {'custom-sa' => {'__GroupVars__' => {'type' => {'_default' => 'apps-service-accounts', 'prod' => 'apps-configmaps'}},
    'spoof' => sa}}, expected_error: 'E_UNEXPECTED_LIST')
  bad = service_account
  bad['roles']['reader']['unrelated'] = ['get']
  render(chart, {'apps-service-accounts' => {'app' => bad}}, expected_error: 'E_UNEXPECTED_LIST')
  render(chart, {'apps-stateless' => {'parent' => {'enabled' => true,
    'containers' => {'main' => {'image' => {'name' => 'nginx', 'staticTag' => '1.27'}}},
    'childApps' => {'apps-configmaps' => {'__GroupVars__' => {'type' => 'apps-service-accounts'}, 'spoof' => sa}}}}},
    expected_error: 'E_UNEXPECTED_LIST')
end
cases['ingress-release'] = lambda do |chart|
  values = {'global' => {'env' => 'prod', 'deploy' => {'enabled' => true, 'release' => 'v1', 'annotateAllWithRelease' => true},
    'releases' => {'v1' => {}}}, 'apps-ingresses' => {'web' => ingress.merge('annotations' => 'custom: preserved')}}
  docs = render(chart, values)
  annotations = resource(docs, 'Ingress').dig('metadata', 'annotations')
  assert(annotations['helm-apps/release'] == 'v1', 'Ingress release annotation missing')
  assert(annotations['custom'] == 'preserved', 'user annotation lost')
  values['global']['deploy']['enabled'] = false
  docs = render(chart, values)
  assert(!resource(docs, 'Ingress').dig('metadata', 'annotations').key?('helm-apps/release'), 'disabled release added annotation')
end
cases['certificate-release'] = lambda do |chart|
  web = ingress
  web['tls'].delete('secret_name')
  docs = render(chart, {'global' => {'env' => 'prod', 'deploy' => {'enabled' => true, 'release' => 'v1', 'annotateAllWithRelease' => true},
    'releases' => {'v1' => {}}}, 'apps-ingresses' => {'web' => web}})
  assert(resource(docs, 'Certificate').dig('metadata', 'annotations', 'helm-apps/release') == 'v1', 'Certificate release annotation missing')
end

selected = ARGV.empty? ? cases.keys : ARGV
unknown = selected - cases.keys
abort "Unknown resource cases: #{unknown.join(', ')}" unless unknown.empty?
failures = []
Dir.mktmpdir('helm-apps-resources-') do |tmp|
  chart = File.join(tmp, 'tests/regressions/resources')
  FileUtils.mkdir_p(File.dirname(chart))
  FileUtils.cp_r(File.join(ROOT, 'tests/regressions/resources'), chart)
  FileUtils.mkdir_p(File.join(tmp, 'charts'))
  FileUtils.cp_r(File.join(ROOT, 'charts/helm-apps'), File.join(tmp, 'charts/helm-apps'))
  # Preserve relative links used by the packaged offline documentation.
  FileUtils.cp_r(File.join(ROOT, 'docs'), File.join(tmp, 'docs'))
  FileUtils.cp(File.join(ROOT, 'tests/.helm/values.schema.json'), File.join(chart, 'values.schema.json'))
  out, err, status = Open3.capture3('helm', 'dependency', 'build', chart, '--skip-refresh')
  abort "dependency build failed: #{out}#{err}" unless status.success?
  selected.each do |name|
    begin
      cases.fetch(name).call(chart)
      puts "PASS resources/#{name}"
    rescue StandardError => e
      failures << name
      warn "FAIL resources/#{name}: #{e.message}"
    end
  end
end
abort "#{failures.size}/#{selected.size} resource regressions failed" unless failures.empty?
puts "#{selected.size}/#{selected.size} resource regressions passed"
