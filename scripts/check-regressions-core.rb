#!/usr/bin/env ruby
require 'fileutils'
require 'open3'
require 'tmpdir'
require 'yaml'

ROOT = File.expand_path('..', __dir__)
failures = []

# Mirror only the local dependency and consumer into a disposable directory.
# Dependency building never changes source Chart.yaml, locks or archives.
Dir.mktmpdir('helm-apps-core-') do |tmp|
  FileUtils.mkdir_p(File.join(tmp, 'charts'))
  FileUtils.mkdir_p(File.join(tmp, 'tests/regressions'))
  FileUtils.cp_r(File.join(ROOT, 'charts/helm-apps'), File.join(tmp, 'charts'))
  FileUtils.cp_r(File.join(ROOT, 'tests/regressions/core'), File.join(tmp, 'tests/regressions'))
  chart = File.join(tmp, 'tests/regressions/core')
  output, status = Open3.capture2e('helm', 'dependency', 'build', chart, '--skip-refresh')
  abort output unless status.success?

  render = lambda do |fixture, *args|
    Open3.capture3('helm', 'template', 'core', chart, '--values', File.join(chart, fixture), *args)
  end
  check = lambda do |name, &test|
    test.call
    puts "PASS #{name}"
  rescue StandardError => e
    failures << name
    warn "FAIL #{name}: #{e.message}"
  end
  manifests = lambda do |fixture, *args|
    out, err, status = render.call(fixture, *args)
    raise err unless status.success?
    YAML.load_stream(out).compact
  end
  find = lambda do |docs, kind, name|
    docs.find { |doc| doc['kind'] == kind && doc.dig('metadata', 'name') == name } ||
      raise("missing #{kind}/#{name}; got #{docs.map { |d| [d['kind'], d.dig('metadata', 'name')] }.inspect}")
  end

  check.call('childApps restores sibling renderer and group context') do
    docs = manifests.call('child-values.yaml')
    find.call(docs, 'Deployment', 'a')
    child = find.call(docs, 'ConfigMap', 'a-config')
    raise 'wrong parent context' unless child.dig('data', 'parent') == 'a'
    sibling = find.call(docs, 'Deployment', 'b')
    annotations = sibling.dig('metadata', 'annotations')
    expected = {'context-group' => 'apps-stateless', 'context-type' => 'apps-stateless', 'context-parent' => 'false'}
    raise "wrong sibling context: #{annotations.inspect}" unless expected.all? { |key, value| annotations[key] == value }
    raise 'unexpected resources' unless docs.length == 3
  end

  [['file-data-values.yaml', 'E_INCLUDE_FROM_FILE'], ['file-malformed-profile-values.yaml', 'E_INCLUDE_FILES_PARSE']].each do |fixture, code|
    %w[broken.yaml broken-directive.yaml list.yaml scalar.yaml null.yaml].each do |file|
      check.call("#{fixture} rejects #{file}") do
        override = fixture == 'file-data-values.yaml' ? 'apps-configmaps.example.data._include_from_file' : 'apps-configmaps.example._include_files[0]'
        out, err, status = render.call(fixture, '--set-string', "#{override}=#{file}")
        raise "expected #{code}, got success: #{out}" if status.success?
        raise "wrong error: #{err}" unless err.include?("[helm-apps:#{code}]") && err.include?(file)
      end
    end
  end

  check.call('_include_files initializes optional include registry') do
    doc = find.call(manifests.call('file-profile-values.yaml'), 'ConfigMap', 'example')
    raise "profile data missing: #{doc.inspect}" unless doc.dig('data', 'key') == 'from-file'
  end
  check.call('_include_from_file permits a user-defined Error key') do
    doc = find.call(manifests.call('file-data-values.yaml', '--set-string', 'apps-configmaps.example.data._include_from_file=map-error.yaml'), 'ConfigMap', 'example')
    raise 'Error data key changed' unless doc.dig('data', 'Error') == 'user-defined-error-value'
  end
  check.call('_include_from_file accepts document markers and local overrides') do
    doc = find.call(manifests.call('file-data-values.yaml', '--set-string', 'apps-configmaps.example.data._include_from_file=map-document.yaml', '--set-string', 'apps-configmaps.example.data.key=local'), 'ConfigMap', 'example')
    raise 'local override lost' unless doc.dig('data', 'key') == 'local'
    raise 'Error data key changed' unless doc.dig('data', 'Error') == 'user-defined-error-value'
  end
  check.call('_include_files accepts document markers and a user-defined Error key') do
    doc = find.call(manifests.call('file-profile-values.yaml', '--set-string', 'apps-configmaps.example._include_files[0]=profile-document.yaml'), 'ConfigMap', 'example')
    raise 'profile data missing' unless doc.dig('data', 'key') == 'from-document'
  end
  %w[map-parser-error.yaml map-parser-error-json.yaml map-parser-error-document.yaml map-parser-error-directive.yaml map-parser-error-inline.yaml map-parser-error-prolog.yaml map-parser-error-literal.yaml].each do |file|
    [['file-data-values.yaml', 'apps-configmaps.example.data._include_from_file'], ['file-data-profile-values.yaml', 'apps-configmaps.example.data._include_files[0]']].each do |fixture, override|
      check.call("#{override} permits literal parser diagnostic in #{file}") do
        doc = find.call(manifests.call(fixture, '--set-string', "#{override}=#{file}"), 'ConfigMap', 'example')
        expected = file == 'map-parser-error-json.yaml' ? 'error unmarshaling JSON: diagnostic from an upstream parser' : 'error converting YAML to JSON: diagnostic from an upstream parser'
        raise "literal Error changed: #{doc.inspect}" unless doc.dig('data', 'Error') == expected
      end
    end
  end
  check.call('_include_from_file accepts an empty YAML map') do
    doc = find.call(manifests.call('file-data-values.yaml', '--set-string', 'apps-configmaps.example.data._include_from_file=empty-map.yaml'), 'ConfigMap', 'example')
    raise 'empty map leaked into data' unless doc.fetch('data', {}).empty?
  end
  check.call('_include_files accepts an empty YAML map') do
    doc = find.call(manifests.call('file-data-profile-values.yaml', '--set-string', 'apps-configmaps.example.data._include_files[0]=empty-map.yaml'), 'ConfigMap', 'example')
    raise 'empty map leaked into data' unless doc.fetch('data', {}).empty?
  end
  %w[missing.yaml empty.yaml].each do |file|
    check.call("optional _include_from_file skips #{file}") do
      doc = find.call(manifests.call('file-data-values.yaml', '--set-string', "apps-configmaps.example.data._include_from_file=#{file}"), 'ConfigMap', 'example')
      raise 'empty include leaked into data' unless doc.fetch('data', {}).empty?
    end
    check.call("optional _include_files skips #{file}") do
      doc = find.call(manifests.call('file-malformed-profile-values.yaml', '--set-string', "apps-configmaps.example._include_files[0]=#{file}"), 'ConfigMap', 'example')
      raise 'empty profile leaked into data' unless doc.fetch('data', {}).empty?
    end
  end
  check.call('werf.env works without repo') do
    doc = find.call(manifests.call('werf-values.yaml'), 'ConfigMap', 'example')
    raise 'unexpected repo label' if doc.dig('metadata', 'labels').key?('repo')
    raise 'wrong config data' unless doc.dig('data', 'key') == 'value'
  end
  check.call('werf.repo keeps existing label') do
    doc = find.call(manifests.call('werf-values.yaml', '--set-string', 'werf.repo=registry.example/team/project'), 'ConfigMap', 'example')
    raise 'repo label changed' unless doc.dig('metadata', 'labels', 'repo') == 'team-project'
  end
end
abort "#{failures.length} core regressions failed" unless failures.empty?
puts 'All core regressions passed'
