#!/usr/bin/env ruby
# Exercise the source library through its only supported consumer entrypoint.
# Packaging and values are isolated: no dependency builds or tracked mutations.
require 'yaml'
require 'tmpdir'
require 'fileutils'
require 'open3'

ROOT = File.expand_path('..', __dir__)
FILTER = ARGV.first

def assert(condition, message)
  raise message unless condition
end

def workload(group = 'apps-stateless', app = {}, container = {})
  { 'global' => { 'env' => 'production' }, group => {
    'review' => { 'enabled' => true, 'containers' => { 'main' => {
      'image' => { 'name' => 'alpine', 'staticTag' => '3' }
    }.merge(container) } }.merge(app)
  } }
end

def object(docs, kind)
  docs.find { |doc| doc['kind'] == kind } || raise("missing #{kind}")
end

def pod(docs, kind = 'Deployment')
  spec = object(docs, kind).fetch('spec')
  spec = spec.fetch('jobTemplate').fetch('spec') if kind == 'CronJob'
  spec.fetch('template').fetch('spec')
end

def render(values)
  File.write(File.join(@chart, 'case.yaml'), YAML.dump(values))
  out, err, status = Open3.capture3('helm', 'template', 'regressions', @chart,
                                  '-f', File.join(@chart, 'case.yaml'), '--kube-version', '1.29.0')
  raise "Helm render failed: #{err.strip}" unless status.success?
  YAML.load_stream(out).compact.select { |doc| doc.is_a?(Hash) && doc['kind'] }
end

tests = {}
%w[Job CronJob].each do |kind|
  tests["#{kind} ServiceAccount"] = proc do
    app = { 'serviceAccount' => { 'enabled' => true, 'name' => { '_default' => 'reviewer' } } }
    app['restartPolicy'] = 'Never'
    app['schedule'] = '*/5 * * * *' if kind == 'CronJob'
    docs = render(workload(kind == 'Job' ? 'apps-jobs' : 'apps-cronjobs', app))
    assert(object(docs, 'ServiceAccount').dig('metadata', 'name') == 'reviewer', 'SA name unresolved')
    assert(pod(docs, kind)['serviceAccountName'] == 'reviewer', 'pod has no serviceAccountName')
  end
end
{ 'env' => { '_default' => 'app: {{ $.CurrentApp.name }}' },
  'tpl' => 'app: {{ $.CurrentApp.name }}' }.each do |mode, selector|
  tests["selector #{mode}"] = proc do
    docs = render(workload('apps-stateless', { 'selector' => selector,
      'podDisruptionBudget' => { 'enabled' => true, 'minAvailable' => 1 } }))
    %w[Deployment PodDisruptionBudget].each do |kind|
      assert(object(docs, kind).dig('spec', 'selector', 'matchLabels') == { 'app' => 'review' }, "#{kind} selector unresolved")
    end
  end
end
tests['external file names'] = proc do
  container = {
    'configFiles' => { 'app.conf' => { 'name' => { '_default' => 'existing-config' }, 'mountPath' => '/app.conf' } },
    'configFilesYAML' => { 'app.yaml' => { 'name' => 'existing-{{ $.CurrentApp.name }}', 'mountPath' => '/app.yaml' } },
    'secretConfigFiles' => { 'secret.conf' => { 'name' => { '_default' => 'existing-secret-{{ $.CurrentApp.name }}' }, 'mountPath' => '/secret.conf' } }
  }
  docs = render(workload('apps-stateless', {}, container))
  names = pod(docs).fetch('volumes').map { |v| v.dig('configMap', 'name') || v.dig('secret', 'secretName') }
  assert(names.sort == %w[existing-config existing-review existing-secret-review].sort, "external names unresolved: #{names}")
  assert(docs.none? { |d| %w[ConfigMap Secret].include?(d['kind']) }, 'external files created managed objects')
end
tests['managed file names'] = proc do
  file = { 'content' => 'managed-content', 'name' => '{{ fail "managed name must be ignored" }}', 'mountPath' => '/app.conf' }
  yaml_file = file.merge('content' => { 'keep' => { '_default' => 'value' } }, 'mountPath' => '/app.yaml')
  docs = render(workload('apps-stateless', {}, { 'configFiles' => { 'app.conf' => file },
    'configFilesYAML' => { 'app.yaml' => yaml_file }, 'secretConfigFiles' => { 'secret.conf' => file } }))
  references = pod(docs).fetch('volumes').map { |v| v.dig('configMap', 'name') || v.dig('secret', 'secretName') }
  names = docs.select { |doc| %w[ConfigMap Secret].include?(doc['kind']) }.map { |doc| doc.dig('metadata', 'name') }
  assert(references.sort == names.sort, 'managed volume names do not match created objects')
end
tests['container volumes'] = proc do
  init = { 'image' => { 'name' => 'alpine', 'staticTag' => '3' }, 'volumes' => "- name: init-data\n  emptyDir: {}\n" }
  docs = render(workload('apps-stateless', {
    'volumes' => "- name: app-data\n  emptyDir: {}\n", 'initContainers' => { 'prepare' => init,
      'disabled' => init.merge('enabled' => false, 'volumes' => "- name: disabled\n  emptyDir: {}\n") }
  }, { 'volumes' => { '_default' => "- name: {{ $.CurrentApp.name }}-data\n  emptyDir: {}\n" },
       'volumeMounts' => "- name: review-data\n  mountPath: /data\n" }))
  names = pod(docs).fetch('volumes').map { |v| v.fetch('name') }
  assert(names.sort == %w[app-data init-data review-data].sort, "missing container volumes: #{names}")
end
tests['duplicate volumes'] = proc do
  begin
    render(workload('apps-stateless', { 'volumes' => "- name: data\n  emptyDir: {}\n" },
      { 'volumes' => "- name: data\n  emptyDir: {}\n" }))
  rescue => e
    assert(e.message.include?('E_VOLUME_NAME_CONFLICT'), "wrong duplicate volume error: #{e.message}")
    next
  end
  raise 'duplicate pod volumes accepted'
end
tests['container volume template context'] = proc do
  container = { 'image' => { 'name' => 'alpine', 'staticTag' => '3' },
    'volumes' => "- name: {{ $.CurrentContainer.name }}-data\n  emptyDir: {}\n",
    'volumeMounts' => "- name: {{ $.CurrentContainer.name }}-data\n  mountPath: /data\n" }
  values = workload('apps-stateless', { 'initContainers' => { 'prepare' => Marshal.load(Marshal.dump(container)) },
    'containers' => { 'first' => Marshal.load(Marshal.dump(container)), 'second' => Marshal.load(Marshal.dump(container)) } })
  spec = pod(render(values))
  names = spec.fetch('volumes').map { |volume| volume.fetch('name') }
  assert(names.sort == %w[first-data prepare-data second-data], "container context lost: #{names}")
  (spec.fetch('containers') + spec.fetch('initContainers')).each do |entry|
    assert(entry.fetch('volumeMounts').first.fetch('name') == "#{entry.fetch('name')}-data", 'volume/mount contexts differ')
  end
end
tests['alwaysRestart'] = proc do
  docs = render(workload('apps-stateless', { 'alwaysRestart' => true }))
  env = pod(docs).fetch('containers').first.fetch('env')
  assert(env.any? { |v| v['name'] == 'FL_APP_ALWAYS_RESTART' && v['value'].size == 20 }, 'restart env absent')
end
tests['envYAML scalar override'] = proc do
  docs = render(workload('apps-stateless', {}, { 'envVars' => { 'FOO' => 'local' },
    'envYAML' => { 'foo' => { '_default' => 'base' }, 'bar' => { '_default' => 'fallback' } } }))
  env = pod(docs).fetch('containers').first.fetch('env').to_h { |v| [v['name'], v['value']] }
  assert(env['FOO'] == 'local' && env['BAR'] == 'fallback', "wrong scalar override: #{env}")
end
tests['envYAML map override'] = proc do
  docs = render(workload('apps-stateless', {}, { 'envVars' => { 'FOO' => { 'production' => 'local' } },
    'envYAML' => { 'foo' => { '_default' => 'base', 'production' => 'base-production' } } }))
  assert(pod(docs).fetch('containers').first.fetch('env').any? { |v| v['name'] == 'FOO' && v['value'] == 'local' }, 'env-map override lost')
end
tests['nested config cleanup'] = proc do
  docs = render(workload('apps-stateless', {}, { 'configFilesYAML' => { 'app.yaml' => {
    'mountPath' => '/app.yaml', 'content' => { 'keep' => { '_default' => 'value' },
      'section' => { '_default' => { 'empty' => {} } } }
  } } }))
  data = YAML.safe_load(object(docs, 'ConfigMap').dig('data', 'app.yaml'))
  assert(data == { 'keep' => 'value' }, "empty nested content survived cleanup: #{data}")
end
tests['secretEnvVars checksum'] = proc do
  values = workload('apps-stateless', {}, { 'secretEnvVars' => { 'TOKEN' => 'old', 'OTHER' => 'constant' } })
  old = render(values)
  values['apps-stateless']['review']['containers']['main']['secretEnvVars']['TOKEN'] = 'new'
  fresh = render(values)
  checksum = proc { |docs| object(docs, 'Deployment').dig('spec', 'template', 'metadata', 'annotations', 'checksum/config') }
  assert(object(old, 'Secret')['data'] != object(fresh, 'Secret')['data'], 'Secret unchanged')
  assert(checksum.call(old) != checksum.call(fresh), 'secret change leaves pod checksum unchanged')
  assert(checksum.call(fresh).match?(/\A[a-f0-9]{64}\z/), 'checksum exposes secret data')
  values['apps-stateless']['review']['containers']['main']['secretEnvVars'] = { 'OTHER' => 'constant', 'TOKEN' => 'new', '__annotations__' => { 'example.com/note' => 'metadata-only' } }
  assert(checksum.call(render(values)) == checksum.call(fresh), 'order or Secret annotation changes checksum')
end
tests['secretEnvVars checksum template context'] = proc do
  values = workload('apps-stateless', {}, { 'secretEnvVars' => { 'TOKEN' => '{{ $.CurrentContainer.name }}' } })
  templated = render(values)
  values['apps-stateless']['review']['containers']['main']['secretEnvVars']['TOKEN'] = 'main'
  literal = render(values)
  assert(object(templated, 'Secret')['data'] == object(literal, 'Secret')['data'], 'equivalent inputs produce different Secret data')
  checksum = proc { |docs| object(docs, 'Deployment').dig('spec', 'template', 'metadata', 'annotations', 'checksum/config') }
  assert(checksum.call(templated) == checksum.call(literal), 'checksum uses different template context than managed Secret')
end
tests['secretEnvVars checksum resource boundaries'] = proc do
  first = { 'image' => { 'name' => 'alpine', 'staticTag' => '3' }, 'secretEnvVars' => { 'A' => '1' } }
  second = { 'image' => { 'name' => 'alpine', 'staticTag' => '3' }, 'secretEnvVars' => { 'B' => '2', 'C' => '3' } }
  values = workload('apps-stateless', { 'containers' => { 'first' => first, 'second' => second } })
  old = render(values)
  first['secretEnvVars']['B'] = second['secretEnvVars'].delete('B')
  fresh = render(values)
  secrets = proc { |docs| docs.select { |doc| doc['kind'] == 'Secret' }.to_h { |doc| [doc.dig('metadata', 'name'), doc['data']] } }
  assert(secrets.call(old) != secrets.call(fresh), 'moving env var did not change Secrets')
  assert(pod(old).fetch('containers') == pod(fresh).fetch('containers'), 'scenario changed Pod container references')
  checksum = proc { |docs| object(docs, 'Deployment').dig('spec', 'template', 'metadata', 'annotations', 'checksum/config') }
  assert(checksum.call(old) != checksum.call(fresh), 'moving env var between Secrets did not change checksum')
end
tests['certificate release annotations'] = proc do
  values = { 'global' => { 'env' => 'production', 'deploy' => {
    'enabled' => true, 'release' => 'v1', 'annotateAllWithRelease' => true }, 'releases' => { 'v1' => {} } },
    'apps-certificates' => { 'review' => { 'enabled' => true, 'host' => 'review.example.com',
      'annotations' => "example.com/owner: test\n" } } }
  annotations = object(render(values), 'Certificate').dig('metadata', 'annotations') || {}
  assert(annotations['helm-apps/release'] == 'v1', 'Certificate lacks release annotation')
  assert(annotations['example.com/owner'] == 'test', 'Certificate custom annotation lost')
end

tests['canonical combined contract fixture'] = proc do
  values = YAML.safe_load(File.read(File.join(ROOT, 'tests/contracts/values.review-regressions.yaml')))
  docs = render(values)
  names = docs.select { |doc| doc['kind'] == 'Deployment' }.map { |doc| doc.dig('metadata', 'name') }
  assert(names.sort == %w[review z-followup], "child renderer leaked into siblings: #{names}")
  review = pod(docs)
  env = review.fetch('containers').first.fetch('env')
  assert(env.any? { |entry| entry['name'] == 'APP_MODE' && entry['value'] == 'local-override' }, 'scalar env override lost')
  assert(review.fetch('volumes').any? { |volume| volume['name'] == 'scratch' }, 'container volume absent')
  runtime = docs.find { |doc| doc['kind'] == 'ConfigMap' && doc.dig('metadata', 'name') == 'review-runtime' }
  assert(runtime && runtime.dig('data', 'parent') == 'review', 'child lost parent context')
end

failures = []
Dir.mktmpdir('helm-apps-workloads-') do |dir|
  @chart = File.join(dir, 'consumer')
  FileUtils.cp_r(File.join(ROOT, 'tests/regressions/workloads'), @chart)
  FileUtils.cp(File.join(ROOT, 'tests/.helm/values.schema.json'), File.join(@chart, 'values.schema.json'))
  FileUtils.mkdir_p(File.join(@chart, 'charts'))
  out, err, status = Open3.capture3('helm', 'package', File.join(ROOT, 'charts/helm-apps'), '--destination', File.join(@chart, 'charts'))
  abort "library package failed: #{out}#{err}" unless status.success?
  tests.each do |name, test|
    next if FILTER && !name.include?(FILTER)
    begin
      test.call
      puts "PASS #{name}"
    rescue => e
      failures << name
      warn "FAIL #{name}: #{e.message}"
    end
  end
end
abort "#{failures.length} workload regression(s) failed" unless failures.empty?
puts 'Workload regressions passed'
