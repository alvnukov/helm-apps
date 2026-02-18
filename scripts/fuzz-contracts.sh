#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

ITERATIONS=40
SEED=20260216

usage() {
  cat <<'EOF'
Usage: scripts/fuzz-contracts.sh [--iterations N] [--seed N]

Property-based stability checks for tests/contracts:
- random toggles for entity enablement and strict/release flags
- random Kubernetes version from compatibility set
- chart must always render successfully

Output is deterministic for the same --seed value.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --iterations)
      ITERATIONS="${2:-}"
      shift 2
      ;;
    --seed)
      SEED="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if ! [[ "${ITERATIONS}" =~ ^[0-9]+$ ]] || [[ "${ITERATIONS}" -le 0 ]]; then
  echo "iterations must be positive integer, got: ${ITERATIONS}" >&2
  exit 2
fi
if ! [[ "${SEED}" =~ ^[0-9]+$ ]]; then
  echo "seed must be integer, got: ${SEED}" >&2
  exit 2
fi

RANDOM="${SEED}"

kube_versions=("1.19.16" "1.20.15" "1.23.17" "1.29.0")

pick_bool() {
  if (( RANDOM % 2 )); then
    echo "true"
  else
    echo "false"
  fi
}

echo "Fuzz contracts: iterations=${ITERATIONS}, seed=${SEED}"

for i in $(seq 1 "${ITERATIONS}"); do
  kv="${kube_versions[$((RANDOM % ${#kube_versions[@]}))]}"
  strict="$(pick_bool)"
  deploy_enabled="$(pick_bool)"

  # Keep at least one workload enabled so manifests are always meaningful.
  enable_stateless="true"
  enable_stateful="$(pick_bool)"
  enable_job="$(pick_bool)"
  enable_cron="$(pick_bool)"
  enable_ingress="$(pick_bool)"
  enable_netpol="$(pick_bool)"
  enable_cilium_netpol="$(pick_bool)"
  enable_calico_netpol="$(pick_bool)"
  enable_configmap="$(pick_bool)"
  enable_secret="$(pick_bool)"
  enable_pvc="$(pick_bool)"
  enable_service="$(pick_bool)"
  enable_limitrange="$(pick_bool)"
  enable_certificate="$(pick_bool)"
  enable_dex_auth="$(pick_bool)"
  enable_dex_client="$(pick_bool)"
  enable_prom_rules="$(pick_bool)"
  enable_dashboard="$(pick_bool)"
  enable_kafka="$(pick_bool)"
  enable_infra_user="$(pick_bool)"
  enable_infra_group="$(pick_bool)"

  out="/tmp/contracts_fuzz_${i}.yaml"
  err="/tmp/contracts_fuzz_${i}.err"

  args=(
    --set "global.env=production"
    --set "global.validation.strict=${strict}"
    --set "global.deploy.enabled=${deploy_enabled}"
    --set "apps-stateless.compat-service.enabled=${enable_stateless}"
    --set "apps-stateful.compat-stateful.enabled=${enable_stateful}"
    --set "apps-jobs.compat-job.enabled=${enable_job}"
    --set "apps-jobs.compat-job.restartPolicy=Never"
    --set "apps-cronjobs.compat-cron.enabled=${enable_cron}"
    --set "apps-cronjobs.compat-cron.restartPolicy=Never"
    --set "apps-ingresses.compat-ingress.enabled=${enable_ingress}"
    --set "apps-network-policies.compat-netpol.enabled=${enable_netpol}"
    --set "apps-network-policies.compat-cilium-netpol.enabled=${enable_cilium_netpol}"
    --set "apps-network-policies.compat-calico-netpol.enabled=${enable_calico_netpol}"
    --set "apps-configmaps.compat-config.enabled=${enable_configmap}"
    --set "apps-secrets.compat-secret.enabled=${enable_secret}"
    --set "apps-pvcs.compat-pvc.enabled=${enable_pvc}"
    --set "apps-services.compat-standalone-service.enabled=${enable_service}"
    --set "apps-limit-range.compat-limit-range.enabled=${enable_limitrange}"
    --set "apps-certificates.compat-certificate.enabled=${enable_certificate}"
    --set "apps-dex-authenticators.compat-dex-auth.enabled=${enable_dex_auth}"
    --set "apps-dex-clients.compat-dex-client.enabled=${enable_dex_client}"
    --set "apps-custom-prometheus-rules.compat-rules.enabled=${enable_prom_rules}"
    --set "apps-grafana-dashboards.compat-dashboard.enabled=${enable_dashboard}"
    --set "apps-kafka-strimzi.compat-kafka.enabled=${enable_kafka}"
    --set "apps-infra.node-users.compat-user.enabled=${enable_infra_user}"
    --set "apps-infra.node-groups.compat-group.enabled=${enable_infra_group}"
  )

  if ! werf helm template contracts tests/contracts --kube-version "${kv}" "${args[@]}" >"${out}" 2>"${err}"; then
    echo "Fuzz iteration ${i} failed (kube=${kv}, strict=${strict}, deployEnabled=${deploy_enabled})" >&2
    echo "See: ${err}" >&2
    sed -n '1,120p' "${err}" >&2 || true
    exit 1
  fi

  if grep -q "_FLANT_APPS_LIBRARY_VERSION_" "${out}"; then
    echo "Fuzz iteration ${i}: unresolved library version placeholder in output" >&2
    exit 1
  fi
done

echo "Fuzz contracts passed: ${ITERATIONS}/${ITERATIONS}"
