#!/usr/bin/env ruby
# Проверяет учебный consumer в отдельном каталоге, без сети и кластера.
require 'fileutils'
require 'open3'
require 'tmpdir'
require 'yaml'

abort('Usage: ruby verify_examples.rb /path/to/helm-apps/charts/helm-apps') unless ARGV.length == 1
library = File.expand_path(ARGV.fetch(0))
skill_root = File.expand_path('..', __dir__)
assets = File.join(skill_root, 'assets', 'consumer')
dependency_version = YAML.safe_load(File.read(File.join(assets, 'Chart.yaml'))).fetch('dependencies').first.fetch('version')
library_version = YAML.safe_load(File.read(File.join(library, 'Chart.yaml'))).fetch('version')
abort("Example pins #{dependency_version}; supplied library is #{library_version}") unless library_version == dependency_version
helm = ['helm']

def check(condition, message)
  raise message unless condition
end

def run_command(args, expect_error: nil)
  output, error, status = Open3.capture3(*args)
  if expect_error
    check(!status.success? && (output + error).include?(expect_error), "Expected #{expect_error}: #{error}")
    return nil
  end
  check(status.success?, "Command failed #{args.inspect}: #{error}")
  output
end

def documents(output)
  YAML.parse_stream(output).children.map do |document|
    stream = Psych::Nodes::Stream.new
    stream.children << document
    YAML.safe_load(stream.to_yaml)
  end.compact
end

def resource(docs, kind, name)
  docs.find { |item| item['kind'] == kind && item.dig('metadata', 'name') == name }
end

def render(helm, chart, env, *extra, expect_error: nil)
  args = helm + ['template', 'demo', chart, '--namespace', 'demo', '--kube-version', '1.29.0', '--set', "global.env=#{env}"] + extra
  output = run_command(args, expect_error: expect_error)
  output && documents(output)
end

def verify_apps(docs, replicas, tags, env)
  replicas.each do |name, expected|
    deployment = resource(docs, 'Deployment', name)
    check(deployment, "Missing Deployment #{name}")
    check(deployment.dig('spec', 'replicas') == expected, "#{name} replicas expected #{expected}")
    container = deployment.dig('spec', 'template', 'spec', 'containers').first
    check(container['image'] == "docker.io/library/nginx:#{tags.fetch(name)}", "#{name} image #{container['image']}")
    check(container.dig('ports', 0, 'containerPort') == 8080, "#{name} port must be integer8080")
    vars = container.fetch('env').to_h { |item| [item.fetch('name'), item['value']] }
    domain = env == 'production' ? 'example.test' : 'dev.example.test'
    check(vars['APP_NAME'] == name && vars['CONTAINER_NAME'] == 'main', "#{name} context")
    check(vars['DOMAIN'] == domain && vars['PUBLIC_URL'] == "https://#{name}.#{domain}", "#{name} env/ref/tpl")
    check(resource(docs, 'Service', name).dig('spec', 'ports', 0, 'port') == 8080, "#{name} service port")
    mounts = container.fetch('volumeMounts')
    pod_volumes = deployment.dig('spec', 'template', 'spec', 'volumes')
    mounts.each do |mount|
      check(pod_volumes.any? { |volume| volume['name'] == mount['name'] }, "#{name} unbound mount")
    end
    configmaps = docs.select { |item| item['kind'] == 'ConfigMap' }
    nginx = configmaps.find { |item| item.dig('data', 'nginx.conf').to_s.include?("#{name} / main") }
    check(nginx && nginx.dig('data', 'nginx.conf').include?('listen 8080;'), "#{name} file tpl")
    configs = configmaps.map do |item|
      text = item.dig('data', 'application.yaml')
      YAML.safe_load(text) if text
    end.compact
    config = configs.find { |item| item.dig('server', 'hostname') == name }
    check(config && config.dig('server', 'timeoutSeconds') == 30, "#{name} structured config context/types")
    expected_peers = env == 'production' ? %w[db-a db-b] : ['localhost']
    check(config['peers'] == expected_peers, "#{name} list env-selection")
    check(env == 'production' ? !config.key?('debug') : config['debug'] == true, "#{name} null/boolean selection")
  end
  check(resource(docs, 'ConfigMap', 'api-runtime').dig('data', 'owner') == 'api', 'Child ParentApp context')
end

Dir.mktmpdir('helm-apps-skill-') do |dir|
  chart = File.join(dir, 'chart')
  FileUtils.cp_r(assets, chart)
  FileUtils.mkdir_p(File.join(chart, 'charts'))
  FileUtils.cp_r(library, File.join(chart, 'charts', 'helm-apps'))
  production = File.join(chart, 'values', 'production.yaml')
  canary = File.join(chart, 'values', 'canary.yaml')
  advanced = File.join(chart, 'values', 'advanced.yaml')
  release = File.join(chart, 'values', 'release.yaml')
  run_command(helm + ['lint', chart, '--set', 'global.env=production'])

  [%w[dev 1], %w[production 3], %w[stage-feature 2]].each do |env, count|
    docs = render(helm, chart, env)
    verify_apps(docs, { 'api' => count.to_i, 'admin' => count.to_i }, { 'api' => '1.27', 'admin' => '1.27' }, env)
    puts "PASS #{env}: includes, env/ref/tpl, files, child, typed config"
  end

  overlay = render(helm, chart, 'production', '-f', production)
  verify_apps(overlay, { 'api' => 6, 'admin' => 3 }, { 'api' => '1.28', 'admin' => '1.27' }, 'production')
  layered = render(helm, chart, 'production', '-f', production, '-f', canary)
  verify_apps(layered, { 'api' => 2, 'admin' => 3 }, { 'api' => '1.29', 'admin' => '1.27' }, 'production')
  reversed = render(helm, chart, 'production', '-f', canary, '-f', production)
  check(resource(reversed, 'Deployment', 'api').dig('spec', 'replicas') == 6, 'Rightmost Helm -f priority')
  cli = render(helm, chart, 'production', '-f', production, '--set', 'apps-stateless.api.replicas=7', '--set-string', 'apps-stateless.api.containers.main.image.staticTag=1.31')
  verify_apps(cli, { 'api' => 7, 'admin' => 3 }, { 'api' => '1.31', 'admin' => '1.27' }, 'production')
  puts 'PASS Helm overlays: local app > imported map, rightmost -f, CLI'

  env_overlay = File.join(dir, 'env-overlay.yaml')
  File.write(env_overlay, YAML.dump('apps-stateless' => { 'api' => {
    'replicas' => { '_default' => '$fl.value{global.vars.replicas}', 'production' => 6 },
    'containers' => { 'main' => { 'image' => { 'staticTag' => { '_default' => '1.27', 'production' => '1.28' } } } }
  } }))
  %w[dev production].each do |env|
    docs = render(helm, chart, env, '-f', env_overlay)
    count = env == 'production' ? 3 : 1
    api_count = env == 'production' ? 6 : 1
    api_tag = env == 'production' ? '1.28' : '1.27'
    verify_apps(docs, { 'api' => api_count, 'admin' => count }, { 'api' => api_tag, 'admin' => '1.27' }, env)
    before = render(helm, chart, env)
    if env == 'dev'
      check(docs == before, 'Env-only overlay must preserve every dev manifest')
    else
      unaffected = lambda { |items| items.reject { |item| item['kind'] == 'Deployment' && item.dig('metadata', 'name') == 'api' } }
      check(unaffected.call(docs) == unaffected.call(before), 'Env-only overlay must preserve unrelated production manifests')
    end
  end
  puts 'PASS env branch overlay: scalar ref fallback retained in dev'

  selected = render(helm, chart, 'production', '-f', release)
  check(!resource(selected, 'Deployment', 'admin'), 'Release must not enable absent admin')
  verify_apps(selected, { 'api' => 3 }, { 'api' => '1.30' }, 'production')
  annotations = resource(selected, 'Deployment', 'api').dig('metadata', 'annotations')
  check(annotations['helm-apps/release'] == 'release-2026-10' && annotations['helm-apps/app-version'] == '1.30', 'Release annotations')
  puts 'PASS selective release: auto-enable, staticTag removal, annotations'

  %w[dev production].each do |env|
    docs = render(helm, chart, env, '-f', advanced)
    data = resource(docs, 'ConfigMap', 'endpoint').fetch('data')
    check(data['endpoint'] == (env == 'production' ? 'https://example.test' : 'https://dev.example.test'), 'Custom endpoint env')
    check(data.key?('details') == (env == 'production'), 'Custom fl.isTrue false handling')
    check(!resource(docs, 'ConfigMap', 'disabled'), 'Dispatcher enabled false')
  end
  puts 'PASS custom renderer: env value, boolean, dispatcher enabled'

  render(helm, chart, 'production', '--set', 'global.validation.strict=true', '--set', 'global.validation.validateTplDelimiters=true')
  puts 'PASS explicit validation flags'

  ambiguous = File.join(dir, 'ambiguous.yaml')
  File.write(ambiguous, YAML.dump('global' => { 'vars' => { 'replicas' => { '^stage-.*$' => 2, '^stage-feature$' => 4 } } }))
  render(helm, chart, 'stage-feature', '-f', ambiguous, expect_error: 'E_ENV_REGEX_AMBIGUOUS')
  missing = File.join(dir, 'missing.yaml')
  File.write(missing, YAML.dump('apps-stateless' => { 'api' => { 'replicas' => '$fl.value{global.vars.not_present}' } }))
  render(helm, chart, 'dev', '-f', missing, expect_error: 'E_VALUE_REF_NOT_FOUND')
  FileUtils.rm(File.join(chart, 'profiles', 'http.yaml'))
  render(helm, chart, 'dev', expect_error: 'Required chart file missing or empty: profiles/http.yaml')
  puts 'PASS expected errors: ambiguous env, missing value ref, required file'
end
puts "Verified example with #{helm.join(' ')} and helm-apps#{library_version}. No source chart changes."
