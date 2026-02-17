#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_SNAPSHOT=1
RUN_CONTRACTS=1
RUN_API=1

usage() {
  cat <<'EOF'
Usage: scripts/ci-local.sh [options]

Runs local equivalent of .github/workflows/ci.yml:validate.

Options:
  --skip-snapshot   Skip snapshot checks (legacy tests snapshot and contracts snapshot).
  --skip-contracts  Skip contracts checks.
  --skip-api        Skip Kubernetes API compatibility checks.
  -h, --help        Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-snapshot)
      RUN_SNAPSHOT=0
      shift
      ;;
    --skip-contracts)
      RUN_CONTRACTS=0
      shift
      ;;
    --skip-api)
      RUN_API=0
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

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

need_cmd werf

if ! command -v kubeconform >/dev/null 2>&1; then
  echo "Missing required command: kubeconform" >&2
  echo "Install: https://github.com/yannh/kubeconform" >&2
  exit 1
fi

if [[ "${RUN_SNAPSHOT}" -eq 1 ]] && ! command -v dyff >/dev/null 2>&1; then
  echo "dyff not found: fallback to diff -u for snapshot check."
fi

APPS_VERSION_FILE="charts/helm-apps/templates/_apps-version.tpl"
TESTS_LOCK="tests/.helm/Chart.lock"
CONTRACTS_LOCK="tests/contracts/Chart.lock"

backup_file() {
  local file="$1"
  if [[ -f "${file}" ]]; then
    cp "${file}" "${file}.bak.ci-local"
  fi
}

restore_file() {
  local file="$1"
  if [[ -f "${file}.bak.ci-local" ]]; then
    mv "${file}.bak.ci-local" "${file}"
  fi
}

cleanup() {
  restore_file "${APPS_VERSION_FILE}"
  restore_file "${TESTS_LOCK}"
  restore_file "${CONTRACTS_LOCK}"
}
trap cleanup EXIT

backup_file "${APPS_VERSION_FILE}"
backup_file "${TESTS_LOCK}"
backup_file "${CONTRACTS_LOCK}"

echo "==> Set library version in ${APPS_VERSION_FILE}"
LIB_VERSION="$(sed -n '/version/{s/version: //;p;}' charts/helm-apps/Chart.yaml)"
sed -i.bak "s/_FLANT_APPS_LIBRARY_VERSION_/${LIB_VERSION}/" "${APPS_VERSION_FILE}"
rm -f "${APPS_VERSION_FILE}.bak"

echo "==> Update test chart dependencies"
werf helm dependency update tests/.helm

echo "==> Validate values schema"
werf helm lint tests/.helm --values tests/.helm/values.yaml

if [[ "${RUN_API}" -eq 1 ]]; then
  echo "==> Verify Kubernetes API compatibility"
  werf helm template tests tests/.helm \
    --set "global.env=prod" \
    --set "global._includes.apps-defaults.enabled=true" \
    --kube-version 1.29.0 > /tmp/tests_k8s_129.yaml
  grep -q '^apiVersion: policy/v1$' /tmp/tests_k8s_129.yaml
  grep -q '^apiVersion: batch/v1$' /tmp/tests_k8s_129.yaml
  grep -q '^apiVersion: autoscaling/v2$' /tmp/tests_k8s_129.yaml
  ! grep -q '^apiVersion: policy/v1beta1$' /tmp/tests_k8s_129.yaml
  ! grep -q '^apiVersion: batch/v1beta1$' /tmp/tests_k8s_129.yaml
  ! grep -q '^apiVersion: autoscaling/v2beta2$' /tmp/tests_k8s_129.yaml
  kubeconform -strict -summary -ignore-missing-schemas -kubernetes-version 1.29.0 /tmp/tests_k8s_129.yaml

  werf helm template tests tests/.helm \
    --set "global.env=prod" \
    --set "global._includes.apps-defaults.enabled=true" \
    --kube-version 1.20.15 > /tmp/tests_k8s_120.yaml
  grep -q '^apiVersion: policy/v1beta1$' /tmp/tests_k8s_120.yaml
  grep -q '^apiVersion: batch/v1beta1$' /tmp/tests_k8s_120.yaml
  grep -q '^apiVersion: autoscaling/v2beta2$' /tmp/tests_k8s_120.yaml
  ! grep -q '^apiVersion: autoscaling/v2$' /tmp/tests_k8s_120.yaml
  kubeconform -strict -summary -ignore-missing-schemas -kubernetes-version 1.20.15 /tmp/tests_k8s_120.yaml
fi

if [[ "${RUN_SNAPSHOT}" -eq 1 ]]; then
  echo "==> Render snapshot check"
  (
    cd tests
    if source "$(werf ci-env github --as-file)" >/dev/null 2>&1; then
      echo "Using werf ci-env github context."
    else
      echo "Cannot load werf ci-env github context; using local context."
    fi

    ruby ../scripts/validate-yaml-stream.rb test_render.yaml
    werf render --dev --set "global._includes.apps-defaults.enabled=true" --env=prod | sed '/werf.io\//d' > test_render_check.yaml
    ruby ../scripts/validate-yaml-stream.rb test_render_check.yaml

    if command -v dyff >/dev/null 2>&1; then
      dyff between test_render.yaml test_render_check.yaml | tee /tmp/test_render_check
      check_tests="$(sed 1,7d /tmp/test_render_check | wc -l | tr -d ' ')"
      if [[ "${check_tests}" -gt "7" ]]; then
        echo "Snapshot mismatch: dyff output lines=${check_tests}" >&2
        exit 1
      fi
    else
      diff -u test_render.yaml test_render_check.yaml
    fi
  )
fi

if [[ "${RUN_CONTRACTS}" -eq 1 ]]; then
  echo "==> Entity coverage checks"
  bash scripts/check-entity-coverage.sh

  echo "==> Contract checks"
  if [[ "${RUN_SNAPSHOT}" -eq 1 ]]; then
    bash scripts/check-contracts.sh
  else
    bash scripts/check-contracts.sh --skip-snapshot
  fi

  echo "==> Property-based fuzz checks"
  bash scripts/fuzz-contracts.sh --iterations 20 --seed 20260216
fi

echo "Local CI validate checks passed."
