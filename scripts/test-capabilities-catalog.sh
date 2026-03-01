#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT_DIR/docs/ai/helm-apps-capabilities.prompt.md"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# Regenerate and compare for drift.
bash "$ROOT_DIR/scripts/generate-capabilities-prompt.sh" >/dev/null
cp "$OUT" "$TMP"

git -C "$ROOT_DIR" diff --exit-code -- "$OUT" >/dev/null || {
  echo "capabilities prompt catalog is outdated: $OUT"
  echo "run: bash scripts/generate-capabilities-prompt.sh"
  exit 1
}

# Sanity checks for critical sections.
grep -q '^## Top-Level Values Sections$' "$TMP"
grep -q '^## All Defined Templates (Full Inventory)$' "$TMP"
grep -q '^## Native YAML List Policy (From `apps-compat.assertNoUnexpectedLists`)$' "$TMP"

# Ensure all built-in apps-* groups are present in catalog.
while IFS= read -r group; do
  pattern="$(printf -- '- `%s`' "$group")"
  grep -Fq -- "$pattern" "$TMP" || {
    echo "missing top-level group in catalog: $group"
    exit 1
  }
done < <(jq -r '.properties | keys[] | select(startswith("apps-"))' "$ROOT_DIR/tests/.helm/values.schema.json")

# Ensure each renderer is listed.
while IFS= read -r renderer; do
  pattern="$(printf -- '- `%s`' "$renderer")"
  grep -Fq -- "$pattern" "$TMP" || {
    echo "missing renderer in catalog: $renderer"
    exit 1
  }
done < <(rg 'define\s+"[^"]+\.render"' "$ROOT_DIR/charts/helm-apps/templates" -o -N | sed -E 's/.*"([^"]+)"/\1/' | sort -u)

echo "capabilities catalog test passed"
