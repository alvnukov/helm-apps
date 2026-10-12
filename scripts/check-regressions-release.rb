#!/usr/bin/env ruby
# Exercise short and extended release entries through the consumer entrypoint.
# All packages, values and rendered manifests stay in a disposable chart.
require 'yaml'
require 'tmpdir'
require 'fileutils'
require 'open3'

ROOT = File.expand_path('..', __dir__)
FILTER = ARGV.first

def assert(condition, message)
  raise message unless condition
end

def values(entry, image = {})
  { 'global' => {
      'env' => 'production', 'validation' => { 'strict' => true },
      'deploy' => { 'enabled' => true, 'release' => 'v1', 'autoEnableApps' => true },
      'releases' => { 'v1' => { 'artifact' => entry } },
      'werfReport' => { 'image' => { 'app' => 'fallback.example/team/app:report' } }
    }, 'apps-stateless' => { 'review' => {
      'enabled' => false, 'versionKey' => 'artifact',
      'containers' => { 'main' => { 'image' => {
        'repository' => 'registry.example/team', 'name' => 'app'
      }.merge(image) } }
    } } }
end

def run_render(input, validate_schema: true)
  path = File.join(@chart, 'case.yaml')
  File.write(path, YAML.dump(input))
  schema_path = File.join(@chart, 'values.schema.json')
  schema_backup = File.join(File.dirname(@chart), 'runtime-values.schema.json')
  # Helm 3.15 lacks --skip-schema-validation. Model a schema-less consumer
  # in the disposable chart, then restore its schema even after an error.
  FileUtils.mv(schema_path, schema_backup) unless validate_schema
  Open3.capture3('helm', 'template', 'release-regressions', @chart,
                '-f', path, '--kube-version', '1.29.0')
ensure
  FileUtils.mv(schema_backup, schema_path) if !validate_schema && schema_backup && File.file?(schema_backup)
end

def render(input)
  out, err, status = run_render(input)
  raise "Helm render failed: #{err.strip}" unless status.success?
  YAML.load_stream(out).compact.select { |doc| doc.is_a?(Hash) && doc['kind'] }
end

def deployment(docs, name = 'review')
  docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == name } ||
    raise("missing Deployment/#{name}")
end

def image_of(docs, name = 'review', container = 'main')
  pod = deployment(docs, name).dig('spec', 'template', 'spec')
  pod.fetch('containers').find { |entry| entry['name'] == container }.fetch('image')
end

def annotations(docs, name = 'review')
  deployment(docs, name).dig('metadata', 'annotations') || {}
end

tests = {}
tests['canonical release image fixture and includes'] = proc do
  input = YAML.safe_load(File.read(File.join(ROOT, 'tests/contracts/values.release-images.yaml')))
  docs = render(input)
  assert(image_of(docs, 'release-short') == 'registry.example/team/app:1.0.0', 'short fixture image wrong')
  assert(image_of(docs, 'release-version-only') == 'registry.example/team/app:1.0.0', 'version-only fixture image wrong')
  assert(image_of(docs, 'release-custom') == 'other.example/team/stage/custom:0.0.1', 'included image overrides lost')
end
tests['legacy scalar and env selectors'] = proc do
  [ 'v1.2.3', { '_default' => 'v1.2.3' },
    { '_default' => 'unused', '^prod.*$' => 'v1.2.3' },
    { '_default' => 'unused', '^prod.*$' => 'unused', 'production' => 'v1.2.3' },
    { 'image' => 'unused', 'production' => 'v1.2.3' } ].each do |entry|
    docs = render(values(entry))
    assert(image_of(docs) == 'registry.example/team/app:v1.2.3', "legacy image changed: #{entry}")
    assert(annotations(docs)['helm-apps/app-version'] == 'v1.2.3', 'legacy version annotation changed')
  end
  [ 7, false, nil ].each do |entry|
    assert(render(values(entry)) == render(values({ '_default' => entry })), 'legacy scalar/env equivalence changed')
  end
end

tests['version-only equivalence'] = proc do
  [ 'v1.2.3', { '_default' => 'v1.2.3', 'production' => 'v2.0.0' }, 7, false, nil ].each do |version|
    assert(render(values(version)) == render(values({ 'version' => version })), "version-only changed manifests: #{version.inspect}")
  end
  assert(render(values({ 'version' => 'v1.2.3', 'image' => {} })) == render(values('v1.2.3')), 'empty image changed defaults')
end

tests['custom repository and path'] = proc do
  entry = { 'version' => '0.0.1', 'image' => {
    'repository' => 'harbor.vimpelcom.ru/vega', 'name' => 'stage/project-cost'
  } }
  docs = render(values(entry))
  assert(image_of(docs) == 'harbor.vimpelcom.ru/vega/stage/project-cost:0.0.1', 'custom ref not preserved exactly')
  assert(annotations(docs)['helm-apps/app-version'] == '0.0.1', 'extended entry leaked into app-version')
  assert(annotations(docs)['helm-apps/release'] == 'v1', 'release annotation lost')
end

tests['partial image overrides'] = proc do
  { { 'name' => 'stage/other' } => 'registry.example/team/stage/other:1.2.3',
    { 'repository' => 'other.example:5000/namespace' } => 'other.example:5000/namespace/app:1.2.3',
    { 'repository' => '' } => 'app:1.2.3' }.each do |override, expected|
    docs = render(values({ 'version' => '1.2.3', 'image' => override }))
    assert(image_of(docs) == expected, "partial override wrong: #{image_of(docs)}")
  end
end

tests['extended env and template resolution'] = proc do
  entry = { 'version' => { '_default' => 'unused', '^prod.*$' => 'unused', 'production' => '0.0.1' },
    'image' => {
      'repository' => { '_default' => 'unused', '^prod.*$' => '$fl.value{global.vars.registry}' },
      'name' => { '_default' => 'unused', 'production' => 'stage/{{ $.CurrentContainer.name }}' }
    } }
  input = values(entry)
  input['global']['vars'] = { 'registry' => 'other.example/team' }
  assert(image_of(render(input)) == 'other.example/team/stage/main:0.0.1', 'extended env/tpl resolution wrong')
end

tests['reserved environment selector nests inside version'] = proc do
  input = values({ 'version' => { '_default' => 'unused', 'version' => '0.0.1' } })
  input['global']['env'] = 'version'
  assert(image_of(render(input)) == 'registry.example/team/app:0.0.1', 'nested reserved environment selector failed')
end

tests['pinned and init images never evaluate release image templates'] = proc do
  input = values({ 'version' => '0.0.1', 'image' => {
    'repository' => '{{ fail "release repository must not resolve" }}',
    'name' => '{{ fail "release name must not resolve" }}'
  } }, { 'staticTag' => 'pinned' })
  input['apps-stateless']['review']['initContainers'] = { 'prepare' => { 'image' => { 'name' => 'app' } } }
  docs = render(input)
  assert(image_of(docs) == 'registry.example/team/app:pinned', 'release image template evaluated for staticTag')
  init = deployment(docs).dig('spec', 'template', 'spec', 'initContainers').first
  assert(init['image'] == 'fallback.example/team/app:report', 'release image template evaluated for init')
end

tests['mixed entries and sibling isolation'] = proc do
  input = values({ 'version' => '0.0.1', 'image' => { 'name' => 'stage/custom' } })
  app = input['apps-stateless'].delete('review')
  input['apps-stateless'] = {
    'a-custom' => app,
    'b-short' => Marshal.load(Marshal.dump(app)).merge('versionKey' => 'short'),
    'c-empty' => Marshal.load(Marshal.dump(app)).merge('enabled' => true, 'versionKey' => 'empty'),
    'd-absent' => Marshal.load(Marshal.dump(app)).merge('enabled' => true, 'versionKey' => 'absent')
  }
  input['global']['releases']['v1']['short'] = '2.0.0'
  input['global']['releases']['v1']['empty'] = { 'version' => { 'dev' => 'unused' }, 'image' => { 'name' => 'must-not-leak' } }
  docs = render(input)
  assert(image_of(docs, 'a-custom') == 'registry.example/team/stage/custom:0.0.1', 'custom sibling wrong')
  assert(image_of(docs, 'b-short') == 'registry.example/team/app:2.0.0', 'custom image leaked to short sibling')
  %w[c-empty d-absent].each do |name|
    assert(image_of(docs, name) == 'fallback.example/team/app:report', "custom image leaked to #{name}")
    assert(!annotations(docs, name).key?('helm-apps/app-version'), "version leaked to #{name}")
  end
end

tests['empty version does not auto-enable'] = proc do
  input = values({ 'version' => { 'dev' => '1.0.0' }, 'image' => { 'name' => 'stage/other' } })
  assert(render(input).none? { |doc| doc['kind'] == 'Deployment' }, 'empty env version enabled app')
end

tests['static sidecar and init preserve images'] = proc do
  input = values({ 'version' => '0.0.1', 'image' => { 'repository' => 'other.example/team', 'name' => 'stage/other' } })
  app = input['apps-stateless']['review']
  app['containers']['sidecar'] = { 'image' => { 'repository' => 'tools.example/team', 'name' => 'sidecar', 'staticTag' => '3' } }
  app['initContainers'] = { 'prepare' => { 'image' => { 'name' => 'app' } } }
  docs = render(input)
  assert(image_of(docs) == 'other.example/team/stage/other:0.0.1', 'main override lost')
  assert(image_of(docs, 'review', 'sidecar') == 'tools.example/team/sidecar:3', 'static sidecar overridden')
  init = deployment(docs).dig('spec', 'template', 'spec', 'initContainers').first
  assert(init['image'] == 'fallback.example/team/app:report', 'init image overridden')
  input['apps-stateless']['review']['containers']['main']['image']['staticTag'] = 'pinned'
  assert(image_of(render(input)) == 'registry.example/team/app:pinned', 'staticTag priority changed')
end

tests['disabled release preserves fallback'] = proc do
  [ false, nil, :absent ].each do |enabled|
    input = values({ 'version' => '{{ fail "version must not resolve" }}',
      'image' => { 'name' => '{{ fail "image must not resolve" }}' } })
    enabled == :absent ? input['global']['deploy'].delete('enabled') : input['global']['deploy']['enabled'] = enabled
    input['apps-stateless']['review']['enabled'] = true
    docs = render(input)
    assert(image_of(docs) == 'fallback.example/team/app:report', 'disabled release changed fallback')
    assert(!annotations(docs).key?('helm-apps/app-version'), 'disabled release added app-version')
    assert(!annotations(docs).key?('helm-apps/release'), 'disabled release added release annotation')
    input['werf'] = { 'image' => { 'app' => 'werf.example/team/app:build' } }
    assert(image_of(render(input)) == 'werf.example/team/app:build', 'werf fallback priority changed')
  end
end

tests['auto-enable remains optional'] = proc do
  [ false, nil, :absent ].each do |enabled|
    input = values({ 'version' => '1.2.3' })
    enabled == :absent ? input['global']['deploy'].delete('autoEnableApps') : input['global']['deploy']['autoEnableApps'] = enabled
    assert(render(input).none? { |doc| doc['kind'] == 'Deployment' }, 'extended entry force-enabled app')
  end
end

tests['workload groups and custom group'] = proc do
  { 'apps-stateless' => 'Deployment', 'apps-stateful' => 'StatefulSet',
    'apps-daemonsets' => 'DaemonSet', 'apps-jobs' => 'Job', 'apps-cronjobs' => 'CronJob',
    'custom-workloads' => 'Deployment' }.each do |group, kind|
    input = values({ 'version' => '1.2.3', 'image' => { 'name' => 'stage/other' } })
    apps = input.delete('apps-stateless')
    apps['review']['service'] = { 'name' => 'review' } if kind == 'StatefulSet'
    apps['review']['schedule'] = '*/5 * * * *' if kind == 'CronJob'
    apps['__GroupVars__'] = { 'type' => 'apps-stateless' } if group == 'custom-workloads'
    input[group] = apps
    doc = render(input).find { |entry| entry['kind'] == kind } || raise("missing #{kind}")
    spec = doc.fetch('spec')
    spec = spec.fetch('jobTemplate').fetch('spec') if kind == 'CronJob'
    assert(spec.dig('template', 'spec', 'containers', 0, 'image') == 'registry.example/team/stage/other:1.2.3', "#{group} wrong image")
  end
end

tests['child resource preserves its own version context'] = proc do
  input = values({ 'version' => '1.2.3', 'image' => { 'name' => 'stage/parent' } })
  input['global']['releases']['v1']['child'] = '2.0.0'
  input['apps-stateless']['review']['childApps'] = { 'apps-configmaps' => {
    'child' => { 'enabled' => true, 'data' => { 'parent' => '{{ $.ParentApp.name }}' } }
  } }
  docs = render(input)
  assert(image_of(docs) == 'registry.example/team/stage/parent:1.2.3', 'parent image changed')
  child = docs.find { |doc| doc['kind'] == 'ConfigMap' && doc.dig('metadata', 'name') == 'child' } || raise('missing child ConfigMap')
  assert(child.dig('data', 'parent') == 'review', 'child lost parent context')
  assert(child.dig('metadata', 'annotations', 'helm-apps/app-version') == '2.0.0', 'child inherited parent version')
end

{ 'unknown entry key' => [{ 'version' => '1', 'typo' => 'x' }, 'E_RELEASE_ENTRY_KEY'],
  'invalid version' => [{ 'version' => ['1'] }, 'E_UNEXPECTED_LIST'],
  'nested version map' => [{ 'version' => { 'production' => { 'nested' => '1' } } }, 'E_RELEASE_VALUE_TYPE'],
  'invalid image type' => [{ 'version' => '1', 'image' => 'other' }, 'E_RELEASE_IMAGE_TYPE'],
  'unknown image key' => [{ 'version' => '1', 'image' => { 'staticTag' => '2' } }, 'E_RELEASE_IMAGE_KEY'],
  'invalid image field' => [{ 'version' => '1', 'image' => { 'name' => 42 } }, 'E_RELEASE_VALUE_TYPE'] }.each do |name, (entry, code)|
  tests["reject #{name}"] = proc do
    _, err, status = run_render(values(entry))
    assert(!status.success? && err.include?('schema'), "schema accepted #{name}: #{err}")
    # Exercise runtime validation separately, including schema-less consumers.
    _, err, status = run_render(values(entry), validate_schema: false)
    assert(!status.success? && err.include?("[helm-apps:#{code}]"), "wrong runtime error for #{name}: #{err}")
  end
end

tests['reject empty resolved image name'] = proc do
  _, err, status = run_render(values({ 'version' => '1', 'image' => { 'name' => { 'dev' => 'other' } } }))
  assert(!status.success? && err.include?('[helm-apps:E_RELEASE_IMAGE_EMPTY]'), "empty image name accepted: #{err}")
end

failures = []
passed = 0
Dir.mktmpdir('helm-apps-release-') do |dir|
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
      passed += 1
      puts "PASS #{name}"
    rescue => e
      failures << name
      warn "FAIL #{name}: #{e.message}"
    end
  end
end
abort "#{failures.length} release regression(s) failed" unless failures.empty?
abort 'No release regressions selected' if passed.zero?
puts "Release regressions passed (#{passed} cases)"
