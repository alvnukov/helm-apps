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
  --skip-snapshot   Skip render snapshot diff check.
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

    werf render --dev --set "global._includes.apps-defaults.enabled=true" --env=prod | sed '/werf.io\//d' > test_render_check.yaml

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
  echo "==> Update contract chart dependencies"
  werf helm dependency update tests/contracts

  echo "==> Contract checks"
  werf helm template contracts tests/contracts --set global.env=production > /tmp/contracts_render.yaml
  grep -q '"A": "2"' /tmp/contracts_render.yaml
  grep -q '"LOCAL": "ok"' /tmp/contracts_render.yaml
  grep -q '"key2": "local-value-2"' /tmp/contracts_render.yaml
  grep -q '"key1": "value-1"' /tmp/contracts_render.yaml
  grep -q '"fromBaseA": "A"' /tmp/contracts_render.yaml
  grep -q '"fromBaseB": "B"' /tmp/contracts_render.yaml
  grep -q '"ENV_SWITCH": "override-default"' /tmp/contracts_render.yaml
  werf helm template contracts tests/contracts --set global.env=dev > /tmp/contracts_render_dev.yaml
  grep -q '"ENV_SWITCH": "override-default"' /tmp/contracts_render_dev.yaml
  grep -q 'paused: true' /tmp/contracts_render.yaml
  grep -q 'resizePolicy:' /tmp/contracts_render.yaml
  grep -q 'podFailurePolicy:' /tmp/contracts_render.yaml
  grep -q 'defaultBackend:' /tmp/contracts_render.yaml
  grep -q 'volumeMode: Filesystem' /tmp/contracts_render.yaml
  grep -q 'immutable: true' /tmp/contracts_render.yaml
  grep -q 'stringData:' /tmp/contracts_render.yaml
  grep -q '^apiVersion: networking.k8s.io/v1$' /tmp/contracts_render.yaml
  grep -q '^kind: NetworkPolicy$' /tmp/contracts_render.yaml
  grep -q '^apiVersion: cilium.io/v2$' /tmp/contracts_render.yaml
  grep -q '^kind: CiliumNetworkPolicy$' /tmp/contracts_render.yaml
  grep -q '^apiVersion: projectcalico.org/v3$' /tmp/contracts_render.yaml
  grep -q 'selector: "app == '\''compat-service'\''"' /tmp/contracts_render.yaml
  grep -q 'kubernetes.io/metadata.name: ingress-nginx' /tmp/contracts_render.yaml
  grep -q 'port: 53' /tmp/contracts_render.yaml
  grep -q 'name: "release-auto-app"' /tmp/contracts_render.yaml
  grep -q 'image: alpine:3.19' /tmp/contracts_render.yaml
  grep -q 'helm-apps/release: "production-v1"' /tmp/contracts_render.yaml
  grep -q 'helm-apps/app-version: "3.19"' /tmp/contracts_render.yaml
  grep -q 'name: "compat-route"' /tmp/contracts_render.yaml
  grep -q 'host: "route.example.com"' /tmp/contracts_render.yaml
  grep -q '^kind: StatefulSet$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-stateful"' /tmp/contracts_render.yaml
  grep -q '^kind: CronJob$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-cron"' /tmp/contracts_render.yaml
  grep -q '^kind: Service$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-standalone-service"' /tmp/contracts_render.yaml
  grep -q '^kind: LimitRange$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-limit-range"' /tmp/contracts_render.yaml
  grep -q '^apiVersion: cert-manager.io/v1$' /tmp/contracts_render.yaml
  grep -q '^kind: Certificate$' /tmp/contracts_render.yaml
  grep -q 'name: compat-certificate' /tmp/contracts_render.yaml
  grep -q '^kind: DexAuthenticator$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-dex-auth"' /tmp/contracts_render.yaml
  grep -q '^kind: DexClient$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-dex-client"' /tmp/contracts_render.yaml
  grep -q '^kind: CustomPrometheusRules$' /tmp/contracts_render.yaml
  grep -q 'name: compat-rules' /tmp/contracts_render.yaml
  grep -q '^kind: GrafanaDashboardDefinition$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-dashboard"' /tmp/contracts_render.yaml
  grep -q '^kind: Kafka$' /tmp/contracts_render.yaml
  grep -q 'name: compat-kafka-' /tmp/contracts_render.yaml
  grep -q '^kind: KafkaTopic$' /tmp/contracts_render.yaml
  grep -q 'name: compat-topic' /tmp/contracts_render.yaml
  grep -q '^kind: NodeUser$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-user"' /tmp/contracts_render.yaml
  grep -q '^kind: NodeGroup$' /tmp/contracts_render.yaml
  grep -q 'name: "compat-group"' /tmp/contracts_render.yaml

  werf helm template contracts tests/contracts --set global.env=production --set global.validation.strict=true > /tmp/contracts_render_strict.yaml
  grep -Eq '"custom": ?"ok"|custom: ?"?ok"?' /tmp/contracts_render_strict.yaml
  ! werf helm template contracts tests/contracts \
    --set global.env=production \
    --set global.validation.strict=true \
    --set apps-network-policies.compat-netpol.typoField=1 >/tmp/contracts_render_strict_fail.yaml
  ! werf helm template contracts tests/contracts \
    --set global.env=production \
    --set global.validation.strict=true \
    --set apps-typo.bad.enabled=true >/tmp/contracts_render_strict_top_fail.yaml

  werf helm template contracts tests/contracts --set global.env=production --kube-version 1.29.0 > /tmp/contracts_render_129.yaml
  grep -q 'loadBalancerClass: "internal-vip"' /tmp/contracts_render_129.yaml
  grep -q 'internalTrafficPolicy: "Local"' /tmp/contracts_render_129.yaml

  werf helm template contracts tests/contracts --set global.env=production --kube-version 1.20.15 > /tmp/contracts_render_120.yaml
  ! grep -q 'loadBalancerClass:' /tmp/contracts_render_120.yaml
  ! grep -q 'internalTrafficPolicy:' /tmp/contracts_render_120.yaml
  grep -q 'ipFamilyPolicy: "SingleStack"' /tmp/contracts_render_120.yaml
  grep -q 'allocateLoadBalancerNodePorts: true' /tmp/contracts_render_120.yaml

  werf helm template contracts tests/contracts --set global.env=production --kube-version 1.19.16 > /tmp/contracts_render_119.yaml
  ! grep -q 'loadBalancerClass:' /tmp/contracts_render_119.yaml
  ! grep -q 'internalTrafficPolicy:' /tmp/contracts_render_119.yaml
  ! grep -q 'ipFamilyPolicy:' /tmp/contracts_render_119.yaml
  ! grep -q 'ipFamilies:' /tmp/contracts_render_119.yaml
  ! grep -q 'allocateLoadBalancerNodePorts:' /tmp/contracts_render_119.yaml

  cat > /tmp/contracts_invalid_native_list.yaml <<'EOF'
apps-stateless:
  compat-service:
    service:
      ports:
        - name: http
          port: 80
          targetPort: 8080
EOF
  ! werf helm template contracts tests/contracts \
    --set global.env=production \
    --values /tmp/contracts_invalid_native_list.yaml \
    >/tmp/contracts_invalid_native_list.out 2>/tmp/contracts_invalid_native_list.err
  grep -q "list value is not allowed at Values.apps-stateless.compat-service.service.ports" /tmp/contracts_invalid_native_list.err

  werf helm template contracts tests/contracts \
    --set global.env=production \
    --values tests/contracts/values.internal-compat.yaml > /tmp/contracts_internal_like.yaml
  grep -q 'name: "compat-web"' /tmp/contracts_internal_like.yaml
  grep -q 'image: alpine:1.2.3' /tmp/contracts_internal_like.yaml
  grep -q 'helm-apps/release:' /tmp/contracts_internal_like.yaml
  grep -q 'helm-apps/app-version: "1.2.3"' /tmp/contracts_internal_like.yaml
  grep -q 'name: "compat-route"' /tmp/contracts_internal_like.yaml
  grep -q 'host: "compat.example.com"' /tmp/contracts_internal_like.yaml
fi

echo "Local CI validate checks passed."
