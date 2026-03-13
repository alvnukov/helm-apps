#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/semantic-yaml-diff-zq.sh <expected.yaml> <actual.yaml> [label]

Compares YAML streams semantically using zq:
- ignores document order in stream;
- ignores map key order;
- preserves array order (lists stay strict).
USAGE
}

if [[ $# -lt 2 || $# -gt 3 ]]; then
  usage >&2
  exit 2
fi

EXPECTED_FILE="$1"
ACTUAL_FILE="$2"
LABEL="${3:-yaml-stream}"

if ! command -v zq >/dev/null 2>&1; then
  echo "Missing required command: zq" >&2
  exit 1
fi

NORM_FILTER='
def norm:
  if type == "object" then
    (to_entries
      | sort_by(.key)
      | map({key: .key, value: (.value | norm)})
      | from_entries)
  elif type == "array" then
    map(norm)
  else
    .
  end;
norm
'

EXPECTED_NORM="$(mktemp /tmp/expected-zq-norm.XXXXXX.jsonl)"
ACTUAL_NORM="$(mktemp /tmp/actual-zq-norm.XXXXXX.jsonl)"

cleanup() {
  rm -f "${EXPECTED_NORM}" "${ACTUAL_NORM}"
}
trap cleanup EXIT

zq --input-format yaml --doc-mode all -c "${NORM_FILTER}" "${EXPECTED_FILE}" | LC_ALL=C sort > "${EXPECTED_NORM}"
zq --input-format yaml --doc-mode all -c "${NORM_FILTER}" "${ACTUAL_FILE}" | LC_ALL=C sort > "${ACTUAL_NORM}"

if cmp -s "${EXPECTED_NORM}" "${ACTUAL_NORM}"; then
  echo "Semantic diff OK: ${LABEL}"
  exit 0
fi

echo "Semantic diff failed: ${LABEL}" >&2
zq --diff --input-format json --doc-mode all --diff-format summary "${EXPECTED_NORM}" "${ACTUAL_NORM}" >&2 || true
exit 1
