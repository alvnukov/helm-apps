#!/usr/bin/env ruby
# frozen_string_literal: true

require 'digest'
require 'fileutils'
require 'open3'
require 'shellwords'
require 'tmpdir'

ROOT = File.expand_path('..', __dir__)

def run!(*args, chdir: ROOT)
  stdout, stderr, status = Open3.capture3(*args, chdir: chdir)
  raise "#{args.first} failed (#{status.exitstatus}): #{stderr.lines.last(8).join}" unless status.success?

  stdout
end

def assert!(condition, message)
  raise message unless condition
end

def check_catalog
  Dir.mktmpdir('helm-apps-catalog-regression-') do |tmp|
    roots = %w[first second].map { |name| File.join(tmp, name) }
    roots.each do |root|
      FileUtils.mkdir_p(File.join(root, 'scripts'))
      FileUtils.mkdir_p(File.join(root, 'tests/.helm'))
      FileUtils.mkdir_p(File.join(root, 'charts/helm-apps'))
      FileUtils.cp(File.join(ROOT, 'scripts/generate-capabilities-prompt.sh'), File.join(root, 'scripts'))
      FileUtils.cp(File.join(ROOT, 'tests/.helm/values.schema.json'), File.join(root, 'tests/.helm'))
      FileUtils.cp(File.join(ROOT, 'AGENTS.md'), root)
    end
    FileUtils.cp_r(File.join(ROOT, 'charts/helm-apps/templates'), File.join(roots.first, 'charts/helm-apps'))
    FileUtils.cp_r(File.join(roots.first, 'charts/helm-apps/templates'), File.join(roots.last, 'charts/helm-apps'))
    roots.each { |root| run!('bash', 'scripts/generate-capabilities-prompt.sh', chdir: root) }
    catalogs = roots.map { |root| File.binread(File.join(root, 'docs/ai/helm-apps-capabilities.prompt.md')) }
    assert!(catalogs.first == catalogs.last, 'catalog depends on checkout path or wall-clock time')
    assert!(!catalogs.first.include?('Generated from code on '), 'catalog includes volatile generation time')
    assert!(!catalogs.first.include?(tmp), 'catalog exposes checkout paths')

    output = File.join(tmp, 'custom-catalog.md')
    before = Digest::SHA256.file(File.join(roots.first, 'docs/ai/helm-apps-capabilities.prompt.md')).hexdigest
    run!('bash', 'scripts/generate-capabilities-prompt.sh', output, chdir: roots.first)
    assert!(File.file?(output), 'generator did not honor output path')
    after = Digest::SHA256.file(File.join(roots.first, 'docs/ai/helm-apps-capabilities.prompt.md')).hexdigest
    assert!(before == after, 'custom output generation changed the default catalog')
  end
  before = Digest::SHA256.file(File.join(ROOT, 'docs/ai/helm-apps-capabilities.prompt.md')).hexdigest
  run!('bash', 'scripts/test-capabilities-catalog.sh')
  after = Digest::SHA256.file(File.join(ROOT, 'docs/ai/helm-apps-capabilities.prompt.md')).hexdigest
  assert!(before == after, 'catalog check modified tracked content')
end

def check_fuzz
  legacy_hashes = 2.times.map do
    run!('bash', 'scripts/fuzz-contracts.sh', '--iterations', '3', '--seed', '20260216')
    (1..3).map { |i| Digest::SHA256.file("/tmp/contracts_fuzz_#{i}.yaml").hexdigest }
  end
  assert!(legacy_hashes.first == legacy_hashes.last, 'same fuzz seed did not reproduce manifests')
  Dir.mktmpdir('helm-apps-fuzz-regression-') do |tmp|
    outputs = %w[first second].map { |name| File.join(tmp, name) }
    outputs.each do |output|
      run!('bash', 'scripts/fuzz-contracts.sh', '--iterations', '3', '--seed', '20260216', '--output-dir', output)
    end
    hashes = outputs.map do |output|
      files = Dir[File.join(output, '*.yaml')].sort
      assert!(files.length == 3, 'fuzz did not keep three manifests in its isolated output directory')
      files.map { |file| Digest::SHA256.file(file).hexdigest }
    end
    assert!(hashes.first == hashes.last, 'same fuzz seed did not reproduce manifests')

    fake_bin = File.join(tmp, 'bin')
    FileUtils.mkdir_p(fake_bin)
    File.write(File.join(fake_bin, 'helm'), "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$FUZZ_CAPTURE\"\necho forced-render-failure >&2\nexit 42\n")
    FileUtils.chmod(0o755, File.join(fake_bin, 'helm'))
    failed_output = File.join(tmp, 'failed')
    capture = File.join(tmp, 'failed-args.txt')
    _, stderr, status = Open3.capture3({ 'PATH' => "#{fake_bin}:#{ENV.fetch('PATH')}", 'FUZZ_CAPTURE' => capture },
                                     'bash', 'scripts/fuzz-contracts.sh', '--iterations', '1',
                                     '--seed', '20260216', '--output-dir', failed_output, chdir: ROOT)
    assert!(!status.success? && stderr.include?('forced-render-failure'), 'forced fuzz failure was not detected')
    reproduction = File.join(failed_output, 'contracts_fuzz_1.repro.sh')
    assert!(File.file?(reproduction), 'failed fuzz iteration did not preserve the exact reproduction command')
    source = File.read(reproduction)
    assert!(source.include?('seed=20260216 iteration=1'), 'fuzz reproduction is missing seed/iteration')
    run!('bash', '-n', reproduction)
    command = Shellwords.split(source.lines.last)
    assert!(command[0, 4] == %w[helm template contracts tests/contracts], 'fuzz reproduction has the wrong entrypoint')
    assert!(command.drop(1) == File.readlines(capture, chomp: true), 'fuzz reproduction lost render configuration')
  end
end

def check_package
  Dir.mktmpdir('helm-apps-package-regression-') do |tmp|
    run!('helm', 'package', File.join(ROOT, 'charts/helm-apps'), '--destination', tmp)
    archive = Dir[File.join(tmp, 'helm-apps-*.tgz')].fetch(0)
    files = run!('tar', '-tzf', archive).lines.map(&:strip)
    assert!(files.include?('helm-apps/templates/_apps-utils.tpl'), 'chart archive is missing runtime templates')
    assert!(!files.include?('helm-apps/AGENTS.md'), 'chart archive includes repository-only AGENTS.md')
    assert!(files.none? { |file| file.start_with?('helm-apps/docs/') }, 'chart archive includes repository-only documentation')
  end
end

checks = { 'catalog' => method(:check_catalog), 'fuzz' => method(:check_fuzz), 'package' => method(:check_package) }
selected = ARGV.empty? ? checks.keys : ARGV
unknown = selected - checks.keys
abort "Unknown checks: #{unknown.join(', ')}" unless unknown.empty?

failures = []
selected.each do |name|
  begin
    checks.fetch(name).call
    puts "PASS tooling/#{name}"
  rescue StandardError => e
    failures << name
    warn "FAIL tooling/#{name}: #{e.message}"
  end
end
abort "Tooling regressions failed: #{failures.join(', ')}" unless failures.empty?
