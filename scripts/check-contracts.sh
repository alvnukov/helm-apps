#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_SNAPSHOT=1
APPS_VERSION_FILE="charts/helm-apps/templates/_apps-version.tpl"

usage() {
  cat <<'USAGE'
Usage: scripts/check-contracts.sh [options]

Renders tests/contracts and runs:
- snapshot check for new-features snapshot;
- structural checks for production/dev/strict/kube-version renders;
- negative checks (strict unknown keys, invalid native list);
- internal-like release/deploy flow checks.

Options:
  --skip-snapshot  Skip contracts snapshot comparison.
  -h, --help       Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-snapshot)
      RUN_SNAPSHOT=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if ! command -v helm >/dev/null 2>&1; then
  echo "Missing required command: helm" >&2
  exit 1
fi

if ! command -v ruby >/dev/null 2>&1; then
  echo "Missing required command: ruby" >&2
  exit 1
fi

if ! command -v zq >/dev/null 2>&1; then
  echo "Missing required command: zq" >&2
  exit 1
fi

backup_file() {
  local file="$1"
  if [[ -f "${file}" ]]; then
    cp "${file}" "${file}.bak.check-contracts"
  fi
}

restore_file() {
  local file="$1"
  if [[ -f "${file}.bak.check-contracts" ]]; then
    mv "${file}.bak.check-contracts" "${file}"
  fi
}

cleanup() {
  restore_file "${APPS_VERSION_FILE}"
}
trap cleanup EXIT

backup_file "${APPS_VERSION_FILE}"

echo "==> Set library version in ${APPS_VERSION_FILE}"
LIB_VERSION="$(sed -n '/version/{s/version: //;p;}' charts/helm-apps/Chart.yaml)"
sed -i.bak "s/_FLANT_APPS_LIBRARY_VERSION_/${LIB_VERSION}/" "${APPS_VERSION_FILE}"
rm -f "${APPS_VERSION_FILE}.bak"

echo "==> Validate contracts snapshot YAML"
ruby scripts/validate-yaml-stream.rb tests/contracts/test_render.snapshot.yaml

echo "==> Update contract chart dependencies"
helm dependency update tests/contracts

echo "==> Render contracts (production/dev/strict + kube compatibility)"
helm template contracts tests/contracts --set global.env=production > /tmp/contracts_render.yaml
helm template contracts tests/contracts --set global.env=dev > /tmp/contracts_render_dev.yaml
helm template contracts tests/contracts --set global.env=production --set global.validation.strict=true > /tmp/contracts_render_strict.yaml
helm template contracts tests/contracts --set global.env=production --kube-version 1.29.0 > /tmp/contracts_render_129.yaml
helm template contracts tests/contracts --set global.env=production --kube-version 1.20.15 > /tmp/contracts_render_120.yaml
helm template contracts tests/contracts --set global.env=production --kube-version 1.19.16 > /tmp/contracts_render_119.yaml

echo "==> Validate rendered contracts YAML streams"
ruby scripts/validate-yaml-stream.rb \
  /tmp/contracts_render.yaml \
  /tmp/contracts_render_dev.yaml \
  /tmp/contracts_render_strict.yaml \
  /tmp/contracts_render_129.yaml \
  /tmp/contracts_render_120.yaml \
  /tmp/contracts_render_119.yaml

if [[ "${RUN_SNAPSHOT}" -eq 1 ]]; then
  echo "==> Contracts snapshot check (new features snapshot)"
  helm template contracts tests/contracts --set global.env=production \
    | sed '/werf.io\//d' > /tmp/contracts_snapshot_check.yaml
  ruby scripts/validate-yaml-stream.rb /tmp/contracts_snapshot_check.yaml

  # Keep contracts snapshot stable across library releases:
  # runtime annotation helm-apps/version is expected to change with Chart version.
  sed '/helm-apps\/version:/d' tests/contracts/test_render.snapshot.yaml > /tmp/contracts_snapshot_expected.raw.yaml
  sed '/helm-apps\/version:/d' /tmp/contracts_snapshot_check.yaml > /tmp/contracts_snapshot_check.raw.yaml

  # After removing helm-apps/version, some resources keep an empty metadata.annotations.
  # Normalize it away on both sides so release bumps do not produce false snapshot diffs.
  zq --input-format yaml --doc-mode all --output-format yaml \
    'if (.metadata.annotations == null or .metadata.annotations == {}) then del(.metadata.annotations) else . end' \
    /tmp/contracts_snapshot_expected.raw.yaml > /tmp/contracts_snapshot_expected.normalized.yaml
  zq --input-format yaml --doc-mode all --output-format yaml \
    'if (.metadata.annotations == null or .metadata.annotations == {}) then del(.metadata.annotations) else . end' \
    /tmp/contracts_snapshot_check.raw.yaml > /tmp/contracts_snapshot_check.normalized.yaml

  scripts/semantic-yaml-diff-zq.sh \
    /tmp/contracts_snapshot_expected.normalized.yaml \
    /tmp/contracts_snapshot_check.normalized.yaml \
    "contracts snapshot"
fi

echo "==> Structural contracts checks"
ruby scripts/verify-contracts-structure.rb main \
  --production /tmp/contracts_render.yaml \
  --dev /tmp/contracts_render_dev.yaml \
  --strict /tmp/contracts_render_strict.yaml \
  --k129 /tmp/contracts_render_129.yaml \
  --k120 /tmp/contracts_render_120.yaml \
  --k119 /tmp/contracts_render_119.yaml

echo "==> Release annotate-all option checks"
helm template contracts tests/contracts \
  --set global.env=production \
  --set global.deploy.annotateAllWithRelease=true \
  > /tmp/contracts_render_annotate_all.yaml

ruby - <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_annotate_all.yaml')).compact.select { |doc| doc.is_a?(Hash) }
deployment = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'compat-service' }
abort 'Missing Deployment/compat-service in annotateAllWithRelease render' unless deployment

release = deployment.dig('metadata', 'annotations', 'helm-apps/release')
abort "Expected helm-apps/release=production-v1, got #{release.inspect}" unless release == 'production-v1'

app_version = deployment.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "compat-service must not get helm-apps/app-version, got #{app_version.inspect}" unless app_version.nil?
RUBY

echo "==> Release mode disable checks"
helm template contracts tests/contracts \
  --set global.env=production \
  --set global.deploy.enabled=false \
  --set global.deploy.annotateAllWithRelease=true \
  > /tmp/contracts_render_release_disabled.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_release_disabled.yaml')).compact.select { |doc| doc.is_a?(Hash) }
release_auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-auto-app' }
abort 'release-auto-app must stay disabled when global.deploy.enabled=false' unless release_auto_app.nil?

compat_service = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'compat-service' }
abort 'Missing Deployment/compat-service in release-disabled render' unless compat_service

release = compat_service.dig('metadata', 'annotations', 'helm-apps/release')
abort "compat-service must not get helm-apps/release when global.deploy.enabled=false, got #{release.inspect}" unless release.nil?

app_version = compat_service.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "compat-service must not get helm-apps/app-version when global.deploy.enabled=false, got #{app_version.inspect}" unless app_version.nil?
RUBY

cat > /tmp/contracts_release_image_scenario.yaml <<'YAML'
apps-stateless:
  release-manual-app:
    enabled: true
    versionKey: release-web
    containers:
      main:
        image:
          name: compat-report-only
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
  release-manual-repository-app:
    enabled: true
    versionKey: release-web
    containers:
      main:
        image:
          repository: registry.example/contracts
          name: compat-report-only
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
  release-static-repository-app:
    enabled: true
    versionKey: release-web
    containers:
      main:
        image:
          repository: registry.example/contracts
          name: compat-report-only
          staticTag: "9.9.9"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
apps-jobs:
  release-init-report-app:
    enabled: true
    versionKey: release-web
    containers:
      main:
        image:
          name: alpine
          staticTag: "3"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
    initContainers:
      prepare:
        image:
          name: compat-report-only
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
YAML

echo "==> Release mode default-off checks"
tmp_contracts_release_default_off_dir="$(mktemp -d)"
cp -R tests/contracts/. "${tmp_contracts_release_default_off_dir}/"
ruby -ryaml -e 'path = ARGV[0]; data = YAML.load_file(path); data.fetch("global").fetch("deploy").delete("enabled"); File.write(path, data.to_yaml)' \
  "${tmp_contracts_release_default_off_dir}/values.yaml"

helm template contracts "${tmp_contracts_release_default_off_dir}" \
  --set global.env=production \
  --set global.deploy.annotateAllWithRelease=true \
  --values /tmp/contracts_release_image_scenario.yaml \
  > /tmp/contracts_render_release_enabled_absent.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_release_enabled_absent.yaml')).compact.select { |doc| doc.is_a?(Hash) }
release_auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-auto-app' }
abort 'release-auto-app must stay disabled when global.deploy.enabled is absent' unless release_auto_app.nil?

compat_service = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'compat-service' }
abort 'Missing Deployment/compat-service in release-enabled-absent render' unless compat_service
release = compat_service.dig('metadata', 'annotations', 'helm-apps/release')
abort "compat-service must not get helm-apps/release when global.deploy.enabled is absent, got #{release.inspect}" unless release.nil?
app_version = compat_service.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "compat-service must not get helm-apps/app-version when global.deploy.enabled is absent, got #{app_version.inspect}" unless app_version.nil?

manual_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-manual-app' }
abort 'Missing Deployment/release-manual-app when global.deploy.enabled is absent' unless manual_app
image = manual_app.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected fallback image registry.example/contracts/compat-report-only:2.4.6 when global.deploy.enabled is absent, got #{image.inspect}" unless image == 'registry.example/contracts/compat-report-only:2.4.6'
release = manual_app.dig('metadata', 'annotations', 'helm-apps/release')
abort "release-manual-app must not get helm-apps/release when global.deploy.enabled is absent, got #{release.inspect}" unless release.nil?
app_version = manual_app.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "release-manual-app must not get helm-apps/app-version when global.deploy.enabled is absent, got #{app_version.inspect}" unless app_version.nil?
RUBY

rm -rf "${tmp_contracts_release_default_off_dir}"

helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_release_image_scenario.yaml \
  > /tmp/contracts_render_release_image_enabled.yaml

helm template contracts tests/contracts \
  --set global.env=production \
  --set global.deploy.enabled=false \
  --values /tmp/contracts_release_image_scenario.yaml \
  > /tmp/contracts_render_release_image_disabled.yaml

ruby <<'RUBY'
require 'yaml'

def find_deployment(path, name)
  docs = YAML.load_stream(File.read(path)).compact.select { |doc| doc.is_a?(Hash) }
  docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == name }
end

def find_job(path, name)
  docs = YAML.load_stream(File.read(path)).compact.select { |doc| doc.is_a?(Hash) }
  docs.find { |doc| doc['kind'] == 'Job' && doc.dig('metadata', 'name') == name }
end

enabled = find_deployment('/tmp/contracts_render_release_image_enabled.yaml', 'release-manual-app')
abort 'Missing Deployment/release-manual-app in release-enabled render' unless enabled
enabled_image = enabled.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected release-derived image compat-report-only:3.19, got #{enabled_image.inspect}" unless enabled_image == 'compat-report-only:3.19'

enabled_repository = find_deployment('/tmp/contracts_render_release_image_enabled.yaml', 'release-manual-repository-app')
abort 'Missing Deployment/release-manual-repository-app in release-enabled render' unless enabled_repository
enabled_repository_image = enabled_repository.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected repository-aware release image registry.example/contracts/compat-report-only:3.19, got #{enabled_repository_image.inspect}" unless enabled_repository_image == 'registry.example/contracts/compat-report-only:3.19'

enabled_static_repository = find_deployment('/tmp/contracts_render_release_image_enabled.yaml', 'release-static-repository-app')
abort 'Missing Deployment/release-static-repository-app in release-enabled render' unless enabled_static_repository
enabled_static_repository_image = enabled_static_repository.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected repository-aware static image registry.example/contracts/compat-report-only:9.9.9, got #{enabled_static_repository_image.inspect}" unless enabled_static_repository_image == 'registry.example/contracts/compat-report-only:9.9.9'

enabled_job = find_job('/tmp/contracts_render_release_image_enabled.yaml', 'release-init-report-app')
abort 'Missing Job/release-init-report-app in release-enabled render' unless enabled_job
enabled_job_init_image = enabled_job.dig('spec', 'template', 'spec', 'initContainers', 0, 'image')
abort "Expected initContainer werfReport fallback registry.example/contracts/compat-report-only:2.4.6, got #{enabled_job_init_image.inspect}" unless enabled_job_init_image == 'registry.example/contracts/compat-report-only:2.4.6'
enabled_job_main_image = enabled_job.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected main container static image alpine:3, got #{enabled_job_main_image.inspect}" unless enabled_job_main_image == 'alpine:3'

disabled = find_deployment('/tmp/contracts_render_release_image_disabled.yaml', 'release-manual-app')
abort 'Missing Deployment/release-manual-app in release-disabled render' unless disabled
disabled_image = disabled.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected fallback image registry.example/contracts/compat-report-only:2.4.6, got #{disabled_image.inspect}" unless disabled_image == 'registry.example/contracts/compat-report-only:2.4.6'

disabled_repository = find_deployment('/tmp/contracts_render_release_image_disabled.yaml', 'release-manual-repository-app')
abort 'Missing Deployment/release-manual-repository-app in release-disabled render' unless disabled_repository
disabled_repository_image = disabled_repository.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected repository app to keep werfReport fallback registry.example/contracts/compat-report-only:2.4.6, got #{disabled_repository_image.inspect}" unless disabled_repository_image == 'registry.example/contracts/compat-report-only:2.4.6'

disabled_static_repository = find_deployment('/tmp/contracts_render_release_image_disabled.yaml', 'release-static-repository-app')
abort 'Missing Deployment/release-static-repository-app in release-disabled render' unless disabled_static_repository
disabled_static_repository_image = disabled_static_repository.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected repository-aware static image registry.example/contracts/compat-report-only:9.9.9, got #{disabled_static_repository_image.inspect}" unless disabled_static_repository_image == 'registry.example/contracts/compat-report-only:9.9.9'

disabled_job = find_job('/tmp/contracts_render_release_image_disabled.yaml', 'release-init-report-app')
abort 'Missing Job/release-init-report-app in release-disabled render' unless disabled_job
disabled_job_init_image = disabled_job.dig('spec', 'template', 'spec', 'initContainers', 0, 'image')
abort "Expected initContainer werfReport fallback registry.example/contracts/compat-report-only:2.4.6 when release logic disabled, got #{disabled_job_init_image.inspect}" unless disabled_job_init_image == 'registry.example/contracts/compat-report-only:2.4.6'
RUBY

echo "==> Empty env-resolved release version checks"
cat > /tmp/contracts_release_empty_env_version.yaml <<'YAML'
global:
  deploy:
    release:
      _default: production-empty-v1
      production: production-empty-v1
  releases:
    production-empty-v1:
      release-empty-web:
        dev: "9.9.9"
apps-stateless:
  release-empty-auto-app:
    enabled: false
    versionKey: release-empty-web
    containers:
      main:
        image:
          name: compat-report-only
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
  release-empty-manual-app:
    enabled: true
    versionKey: release-empty-web
    containers:
      main:
        image:
          name: compat-report-only
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
YAML

helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_release_empty_env_version.yaml \
  > /tmp/contracts_render_release_empty_env_version.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_release_empty_env_version.yaml')).compact.select { |doc| doc.is_a?(Hash) }
auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-empty-auto-app' }
abort 'release-empty-auto-app must stay disabled when release version resolves to empty for current env' unless auto_app.nil?

manual_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-empty-manual-app' }
abort 'Missing Deployment/release-empty-manual-app in empty resolved version render' unless manual_app

image = manual_app.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected fallback image registry.example/contracts/compat-report-only:2.4.6 for empty resolved version, got #{image.inspect}" unless image == 'registry.example/contracts/compat-report-only:2.4.6'

release = manual_app.dig('metadata', 'annotations', 'helm-apps/release')
abort "release-empty-manual-app must not get helm-apps/release for empty resolved version, got #{release.inspect}" unless release.nil?

app_version = manual_app.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "release-empty-manual-app must not get helm-apps/app-version for empty resolved version, got #{app_version.inspect}" unless app_version.nil?
RUBY

echo "==> Release auto-enable option checks"
helm template contracts tests/contracts \
  --set global.env=production \
  --set global.deploy.autoEnableApps=false \
  > /tmp/contracts_render_auto_enable_disabled.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_auto_enable_disabled.yaml')).compact.select { |doc| doc.is_a?(Hash) }
release_auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-auto-app' }
abort 'release-auto-app must stay disabled when global.deploy.autoEnableApps=false' unless release_auto_app.nil?
RUBY

tmp_contracts_absent_dir="$(mktemp -d)"
cp -R tests/contracts/. "${tmp_contracts_absent_dir}/"
grep -v 'autoEnableApps:' "${tmp_contracts_absent_dir}/values.yaml" > "${tmp_contracts_absent_dir}/values.yaml.tmp"
mv "${tmp_contracts_absent_dir}/values.yaml.tmp" "${tmp_contracts_absent_dir}/values.yaml"

helm template contracts "${tmp_contracts_absent_dir}" \
  --set global.env=production \
  > /tmp/contracts_render_auto_enable_absent.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_auto_enable_absent.yaml')).compact.select { |doc| doc.is_a?(Hash) }
release_auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-auto-app' }
abort 'release-auto-app must stay disabled when global.deploy.autoEnableApps is absent' unless release_auto_app.nil?
RUBY

cat > /tmp/contracts_auto_enable_null.yaml <<'YAML'
global:
  deploy:
    autoEnableApps: null
YAML

helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_auto_enable_null.yaml \
  > /tmp/contracts_render_auto_enable_null.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_auto_enable_null.yaml')).compact.select { |doc| doc.is_a?(Hash) }
release_auto_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-auto-app' }
abort 'release-auto-app must stay disabled when global.deploy.autoEnableApps is null' unless release_auto_app.nil?
RUBY

rm -rf "${tmp_contracts_absent_dir}"

helm template contracts tests/contracts \
  --set global.env=production \
  --set global.deploy.autoEnableApps=false \
  --set global.deploy.annotateAllWithRelease=true \
  --values /tmp/contracts_release_image_scenario.yaml \
  > /tmp/contracts_render_auto_enable_manual.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_auto_enable_manual.yaml')).compact.select { |doc| doc.is_a?(Hash) }
manual_app = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'release-manual-app' }
abort 'Missing Deployment/release-manual-app in autoEnableApps=false render' unless manual_app

image = manual_app.dig('spec', 'template', 'spec', 'containers', 0, 'image')
abort "Expected release image compat-report-only:3.19, got #{image.inspect}" unless image == 'compat-report-only:3.19'

release = manual_app.dig('metadata', 'annotations', 'helm-apps/release')
abort "Expected helm-apps/release=production-v1, got #{release.inspect}" unless release == 'production-v1'

app_version = manual_app.dig('metadata', 'annotations', 'helm-apps/app-version')
abort "Expected helm-apps/app-version=3.19, got #{app_version.inspect}" unless app_version == '3.19'
RUBY

echo "==> childApps checks"
cat > /tmp/contracts_child_apps.yaml <<'YAML'
apps-stateless:
  child-parent-enabled:
    enabled: true
    containers:
      main:
        image:
          name: alpine
          staticTag: "3"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
    childApps:
      apps-configmaps:
        derived-config:
          enabled: true
          name: "{{ $.ParentApp.name }}-config"
          data: |
            parentName: {{ $.ParentApp.name | quote }}
        disabled-by-default:
          name: "{{ $.ParentApp.name }}-default-disabled"
          data: |
            parentName: {{ $.ParentApp.name | quote }}
      apps-ingresses:
        public:
          enabled: true
          name: "{{ $.ParentApp.name }}"
          host: child.example.com
          paths: |
            - path: /
              pathType: Prefix
              backend:
                service:
                  name: compat-service
                  port:
                    number: 80
  child-parent-disabled:
    enabled: false
    containers:
      main:
        image:
          name: alpine
          staticTag: "3"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
    childApps:
      apps-configmaps:
        should-not-render:
          enabled: true
          name: "{{ $.ParentApp.name }}-config"
          data: |
            parentName: {{ $.ParentApp.name | quote }}
YAML

helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --values /tmp/contracts_child_apps.yaml \
  > /tmp/contracts_render_child_apps.yaml

ruby <<'RUBY'
require 'yaml'

docs = YAML.load_stream(File.read('/tmp/contracts_render_child_apps.yaml')).compact.select { |doc| doc.is_a?(Hash) }
parent = docs.find { |doc| doc['kind'] == 'Deployment' && doc.dig('metadata', 'name') == 'child-parent-enabled' }
abort 'Missing Deployment/child-parent-enabled in childApps render' unless parent

config = docs.find { |doc| doc['kind'] == 'ConfigMap' && doc.dig('metadata', 'name') == 'child-parent-enabled-config' }
abort 'Missing ConfigMap/child-parent-enabled-config in childApps render' unless config
parent_name = config.dig('data', 'parentName')
abort "Expected ConfigMap child to see ParentApp.name=child-parent-enabled, got #{parent_name.inspect}" unless parent_name == 'child-parent-enabled'

ingress = docs.find { |doc| doc['kind'] == 'Ingress' && doc.dig('metadata', 'name') == 'child-parent-enabled' }
abort 'Missing Ingress/child-parent-enabled in childApps render' unless ingress
host = ingress.dig('spec', 'rules', 0, 'host')
abort "Expected child ingress host child.example.com, got #{host.inspect}" unless host == 'child.example.com'

default_disabled = docs.find { |doc| doc['kind'] == 'ConfigMap' && doc.dig('metadata', 'name') == 'child-parent-enabled-default-disabled' }
abort 'child app without enabled must keep default disabled semantics' unless default_disabled.nil?

disabled_parent_child = docs.find { |doc| doc['kind'] == 'ConfigMap' && doc.dig('metadata', 'name') == 'child-parent-disabled-config' }
abort 'childApps must not render when parent app is disabled' unless disabled_parent_child.nil?
RUBY

echo "==> childApps negative checks"
cat > /tmp/contracts_child_apps_invalid_group.yaml <<'YAML'
apps-stateless:
  child-parent-invalid:
    enabled: true
    containers:
      main:
        image:
          name: alpine
          staticTag: "3"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
    childApps:
      apps-stateless:
        nested-workload:
          enabled: true
          containers:
            main:
              image:
                name: alpine
                staticTag: "3"
YAML

! helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_child_apps_invalid_group.yaml \
  >/tmp/contracts_child_apps_invalid_group.out 2>/tmp/contracts_child_apps_invalid_group.err

grep -q "\[helm-apps:E_CHILD_APPS_GROUP\]" /tmp/contracts_child_apps_invalid_group.err
grep -q "path=apps-stateless.child-parent-invalid.childApps.apps-stateless" /tmp/contracts_child_apps_invalid_group.err

echo "==> Env label opt-in checks"
helm template contracts tests/contracts \
  --set global.env=production \
  --set global.labels.addEnv=true \
  > /tmp/contracts_render_with_env_label.yaml

ruby <<'RUBY'
require 'yaml'
docs = YAML.load_stream(File.read('/tmp/contracts_render_with_env_label.yaml')).compact
deployment = docs.find { |d| d.is_a?(Hash) && d['kind'] == 'Deployment' && d.dig('metadata', 'name') == 'compat-service' }
abort 'Deployment compat-service not found in env label check output' if deployment.nil?
env_label = deployment.dig('metadata', 'labels', 'app.kubernetes.io/environment')
abort "Expected app.kubernetes.io/environment=production, got #{env_label.inspect}" unless env_label == 'production'
RUBY

echo "==> Strict negative checks"
! helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --set apps-network-policies.compat-netpol.typoField=1 >/tmp/contracts_render_strict_fail.yaml

! helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --set apps-typo.bad.enabled=true >/tmp/contracts_render_strict_top_fail.yaml

! helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --set apps-stateless.compat-service.securityContexts=1 \
  >/tmp/contracts_render_strict_workload_fail.out 2>/tmp/contracts_render_strict_workload_fail.err

grep -q "\[helm-apps:E_STRICT_UNKNOWN_KEY\]" /tmp/contracts_render_strict_workload_fail.err
grep -q "path=apps-stateless.compat-service.securityContexts" /tmp/contracts_render_strict_workload_fail.err

echo "==> Legacy serviceAccount.clusterRole guard checks"
cat > /tmp/contracts_legacy_serviceaccount_clusterrole.yaml <<'YAML'
apps-stateless:
  legacy-rbac-app:
    enabled: true
    containers:
      main:
        image:
          name: alpine
          staticTag: "3"
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
    serviceAccount:
      enabled: true
      name: legacy-rbac-app
      clusterRole:
        name: legacy-rbac-app:reader
        rules: |
          - apiGroups: [""]
            resources: ["pods"]
            verbs: ["get"]
YAML

helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_legacy_serviceaccount_clusterrole.yaml \
  >/tmp/contracts_legacy_serviceaccount_clusterrole_compat.yaml

grep -q 'name: "legacy-rbac-app"' /tmp/contracts_legacy_serviceaccount_clusterrole_compat.yaml
grep -q 'name: "legacy-rbac-app:reader"' /tmp/contracts_legacy_serviceaccount_clusterrole_compat.yaml

! helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.forbidLegacyServiceAccountClusterRole=true \
  --values /tmp/contracts_legacy_serviceaccount_clusterrole.yaml \
  >/tmp/contracts_legacy_serviceaccount_clusterrole_forbidden.out 2>/tmp/contracts_legacy_serviceaccount_clusterrole_forbidden.err

grep -q "\[helm-apps:E_LEGACY_SERVICE_ACCOUNT_CLUSTER_ROLE_FORBIDDEN\]" /tmp/contracts_legacy_serviceaccount_clusterrole_forbidden.err

echo "==> Missing env negative check"
! helm template contracts tests/contracts \
  --set-string global.env= \
  >/tmp/contracts_env_required.out 2>/tmp/contracts_env_required.err
grep -q "\[helm-apps:E_ENV_REQUIRED\]" /tmp/contracts_env_required.err

echo "==> Native list policy negative check"
cat > /tmp/contracts_invalid_native_list.yaml <<'YAML'
apps-stateless:
  native-list-env-optin:
    enabled: false
    containers:
      main:
        command:
          _default: |
            - sh
        args:
          _default: |
            - -c
            - sleep 7200
        ports:
          _default: |
            - name: http
              containerPort: 8089
    service:
      ports:
        _default: |
          - name: http
            port: 8089
            targetPort: 8089
  native-list-optin:
    enabled: false
    containers:
      main:
        command: |
          - sh
        args: |
          - -c
          - sleep 3600
        ports: |
          - name: http
            containerPort: 8088
    service:
      ports: |
        - name: http
          port: 8088
          targetPort: 8088
  compat-native-list-envlike-dev:
    enabled: false
    imagePullSecrets: ""
  compat-native-list-envlike-default:
    enabled: false
    imagePullSecrets: ""
  compat-native-list-literal-tpl:
    enabled: false
    imagePullSecrets: ""
  compat-service:
    service:
      ports:
        - name: http
          port: 80
          targetPort: 8080
YAML

! helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.allowNativeListsInBuiltInListFields=false \
  --values /tmp/contracts_invalid_native_list.yaml \
  >/tmp/contracts_invalid_native_list.out 2>/tmp/contracts_invalid_native_list.err

grep -q "\[helm-apps:E_UNEXPECTED_LIST\]" /tmp/contracts_invalid_native_list.err
grep -q "path=Values.apps-stateless.compat-service.service.ports" /tmp/contracts_invalid_native_list.err

echo "==> Invalid built-in list type negative check"
cat > /tmp/contracts_invalid_list_type.yaml <<'YAML'
apps-stateless:
  compat-service:
    imagePullSecrets: regcred
YAML

! helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_invalid_list_type.yaml \
  >/tmp/contracts_invalid_list_type.out 2>/tmp/contracts_invalid_list_type.err

grep -q "\[helm-apps:E_LIST_FIELD\]" /tmp/contracts_invalid_list_type.err
grep -q "path=apps-stateless.compat-service.imagePullSecrets" /tmp/contracts_invalid_list_type.err

echo "==> Secret config source negative check"
cat > /tmp/contracts_invalid_secret_config_file.yaml <<'YAML'
apps-stateless:
  compat-service:
    containers:
      main:
        secretConfigFiles:
          missing-source.txt:
            mountPath: /etc/missing-source.txt
YAML

! helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_invalid_secret_config_file.yaml \
  >/tmp/contracts_invalid_secret_config_file.out 2>/tmp/contracts_invalid_secret_config_file.err

grep -q "\[helm-apps:E_CONFIG_FILE_SOURCE\]" /tmp/contracts_invalid_secret_config_file.err
grep -q "secretConfigFiles.missing-source.txt must define content or name" /tmp/contracts_invalid_secret_config_file.err

echo "==> TPL delimiter validation opt-in negative check"
cat > /tmp/contracts_invalid_tpl_delimiters.yaml <<'YAML'
global:
  validation:
    validateTplDelimiters: true
apps-configmaps:
  tpl-delim-bad:
    enabled: true
    name: tpl-delim-bad
    data: |
      value: "{{ bad"
YAML

! helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_invalid_tpl_delimiters.yaml \
  >/tmp/contracts_invalid_tpl_delimiters.out 2>/tmp/contracts_invalid_tpl_delimiters.err

grep -q "\[helm-apps:E_TPL_DELIMITERS\]" /tmp/contracts_invalid_tpl_delimiters.err

echo "==> Internal-like release/deploy flow checks"
helm template contracts tests/contracts \
  --set global.env=production \
  --values tests/contracts/values.internal-compat.yaml > /tmp/contracts_internal_like.yaml

ruby scripts/validate-yaml-stream.rb /tmp/contracts_internal_like.yaml
ruby scripts/verify-contracts-structure.rb internal --file /tmp/contracts_internal_like.yaml

echo "Contracts checks passed."
