#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

LIB_FILE="charts/helm-apps/templates/_apps-utils.tpl"
CONTRACTS_FILE="tests/contracts/values.yaml"

if [[ ! -f "${LIB_FILE}" ]]; then
  echo "Missing file: ${LIB_FILE}" >&2
  exit 1
fi
if [[ ! -f "${CONTRACTS_FILE}" ]]; then
  echo "Missing file: ${CONTRACTS_FILE}" >&2
  exit 1
fi

# Read built-in library entity names from the list in init-library template.
mapfile -t entities < <(
  awk '
    /"stateless"/,/"network-policies"/ {
      if ($0 ~ /"[a-z0-9-]+"/) {
        gsub(/^[[:space:]]*"/, "", $0)
        gsub(/".*$/, "", $0)
        print "apps-" $0
      }
    }
  ' "${LIB_FILE}" | sort -u
)

if [[ ${#entities[@]} -eq 0 ]]; then
  echo "Cannot detect library entities from ${LIB_FILE}" >&2
  exit 1
fi

if command -v rg >/dev/null 2>&1; then
  mapfile -t covered < <(
    rg '^apps-[a-z0-9-]+:' "${CONTRACTS_FILE}" -o --replace '$0' \
      | sed 's/:$//' \
      | sort -u
  )
else
  mapfile -t covered < <(
    grep -Eo '^apps-[a-z0-9-]+:' "${CONTRACTS_FILE}" \
      | sed 's/:$//' \
      | sort -u
  )
fi

declare -A covered_map=()
for c in "${covered[@]}"; do
  covered_map["$c"]=1
done

missing=()
for e in "${entities[@]}"; do
  if [[ -z "${covered_map[$e]+x}" ]]; then
    missing+=("$e")
  fi
done

total="${#entities[@]}"
hit=$((total - ${#missing[@]}))
percent=$((100 * hit / total))

echo "Entity coverage: ${hit}/${total} (${percent}%)"
echo "Covered entities:"
printf '  - %s\n' "${covered[@]}"

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "Missing entities in ${CONTRACTS_FILE}:" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  exit 1
fi
