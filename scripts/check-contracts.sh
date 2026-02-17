#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_SNAPSHOT=1

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

if ! command -v werf >/dev/null 2>&1; then
  echo "Missing required command: werf" >&2
  exit 1
fi

if ! command -v ruby >/dev/null 2>&1; then
  echo "Missing required command: ruby" >&2
  exit 1
fi

echo "==> Update contract chart dependencies"
werf helm dependency update tests/contracts

echo "==> Render contracts (production/dev/strict + kube compatibility)"
werf helm template contracts tests/contracts --set global.env=production > /tmp/contracts_render.yaml
werf helm template contracts tests/contracts --set global.env=dev > /tmp/contracts_render_dev.yaml
werf helm template contracts tests/contracts --set global.env=production --set global.validation.strict=true > /tmp/contracts_render_strict.yaml
werf helm template contracts tests/contracts --set global.env=production --kube-version 1.29.0 > /tmp/contracts_render_129.yaml
werf helm template contracts tests/contracts --set global.env=production --kube-version 1.20.15 > /tmp/contracts_render_120.yaml
werf helm template contracts tests/contracts --set global.env=production --kube-version 1.19.16 > /tmp/contracts_render_119.yaml

if [[ "${RUN_SNAPSHOT}" -eq 1 ]]; then
  echo "==> Contracts snapshot check (new features snapshot)"
  werf helm template contracts tests/contracts --set global.env=production \
    | sed '/werf.io\//d' > /tmp/contracts_snapshot_check.yaml
  diff -u tests/contracts/test_render.snapshot.yaml /tmp/contracts_snapshot_check.yaml
fi

echo "==> Structural contracts checks"
ruby scripts/verify-contracts-structure.rb main \
  --production /tmp/contracts_render.yaml \
  --dev /tmp/contracts_render_dev.yaml \
  --strict /tmp/contracts_render_strict.yaml \
  --k129 /tmp/contracts_render_129.yaml \
  --k120 /tmp/contracts_render_120.yaml \
  --k119 /tmp/contracts_render_119.yaml

echo "==> Strict negative checks"
! werf helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --set apps-network-policies.compat-netpol.typoField=1 >/tmp/contracts_render_strict_fail.yaml

! werf helm template contracts tests/contracts \
  --set global.env=production \
  --set global.validation.strict=true \
  --set apps-typo.bad.enabled=true >/tmp/contracts_render_strict_top_fail.yaml

echo "==> Native list policy negative check"
cat > /tmp/contracts_invalid_native_list.yaml <<'YAML'
apps-stateless:
  compat-service:
    service:
      ports:
        - name: http
          port: 80
          targetPort: 8080
YAML

! werf helm template contracts tests/contracts \
  --set global.env=production \
  --values /tmp/contracts_invalid_native_list.yaml \
  >/tmp/contracts_invalid_native_list.out 2>/tmp/contracts_invalid_native_list.err

grep -q "list value is not allowed at Values.apps-stateless.compat-service.service.ports" /tmp/contracts_invalid_native_list.err

echo "==> Internal-like release/deploy flow checks"
werf helm template contracts tests/contracts \
  --set global.env=production \
  --values tests/contracts/values.internal-compat.yaml > /tmp/contracts_internal_like.yaml

ruby scripts/verify-contracts-structure.rb internal --file /tmp/contracts_internal_like.yaml

echo "Contracts checks passed."
