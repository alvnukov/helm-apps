#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCHEMA="$ROOT_DIR/tests/.helm/values.schema.json"
TEMPLATES_DIR="$ROOT_DIR/charts/helm-apps/templates"
OUT="$ROOT_DIR/docs/ai/helm-apps-capabilities.prompt.md"

mkdir -p "$(dirname "$OUT")"

{
  printf '# helm-apps Capability Catalog (Prompt Input)\n\n'
  printf 'Generated from code on %s.\n\n' "$(date -u +'%Y-%m-%d %H:%M:%SZ')"
  printf 'Sources:\n'
  printf -- '- `%s`\n' "$SCHEMA"
  printf -- '- `%s`\n' "$TEMPLATES_DIR"
  printf -- '- `%s`\n\n' "$ROOT_DIR/AGENTS.md"
  printf 'Use this file as machine-oriented context for AI prompts about helm-apps values format and rendering behavior.\n\n'

  printf '## Top-Level Values Sections\n'
  jq -r '.properties | keys[] | "- `" + . + "`"' "$SCHEMA"
  printf '\n'

  printf '## Top-Level Section -> Schema Definition\n'
  jq -r '.properties | to_entries[] | "- `" + .key + "` -> `" + (.value["$ref"] // (.value.type // "object")) + "`"' "$SCHEMA"
  printf '\n'

  printf '## Global Keys\n'
  jq -r '."$defs".global.properties | keys[] | "- `global." + . + "`"' "$SCHEMA"
  printf '\n'

  printf '## Global Validation Flags\n'
  if jq -e '."$defs".global.properties.validation.properties' "$SCHEMA" >/dev/null; then
    jq -r '."$defs".global.properties.validation.properties | keys[] | "- `global.validation." + . + "`"' "$SCHEMA"
  else
    printf -- '- validation flags are dynamically interpreted in templates\n'
  fi
  printf '\n'

  printf '## Common App Model Keys (`$defs.app`)\n'
  jq -r '."$defs".app.properties | keys[] | "- `" + . + "`"' "$SCHEMA"
  printf '\n'

  printf '## Specialized Model Keys\n'
  for def in container image resources resourcesGroup configFile service serviceAccount hpa vpa pdb tls networkPolicyApp appsInfra appsInfraNodeUser customGroupVars; do
    if jq -e --arg def "$def" '."$defs"[$def].properties' "$SCHEMA" >/dev/null; then
      printf '### %s\n' "$def"
      jq -r --arg def "$def" '."$defs"[$def].properties | keys[] | "- `" + . + "`"' "$SCHEMA"
      printf '\n'
    fi
  done

  printf '## Built-In Renderers (`*.render`)\n'
  rg 'define\s+"[^"]+\.render"' "$TEMPLATES_DIR" -o -N \
    | sed -E 's/.*"([^"]+)"/\1/' \
    | sort -u \
    | sed 's/^/- `/' | sed 's/$/`/'
  printf '\n'

  printf '## All Defined Templates (Full Inventory)\n'
  rg 'define\s+"[^"]+"' "$TEMPLATES_DIR" -o -N \
    | sed -E 's/.*"([^"]+)"/\1/' \
    | sort -u \
    | sed 's/^/- `/' | sed 's/$/`/'
  printf '\n'

  printf '## Native YAML List Policy (From `apps-compat.assertNoUnexpectedLists`)\n'
  printf 'Native lists are generally forbidden, except allowed paths/fields detected in code:\n'
  printf -- '- `_include`\n'
  printf -- '- `_include_files`\n'
  printf -- '- `Values.global._includes.*`\n'
  printf -- '- `Values.apps-kafka-strimzi.*.kafka.brokers.hosts.*`\n'
  printf -- '- `Values.apps-kafka-strimzi.*.kafka.ui.dex.allowedGroups.*`\n'
  printf -- '- `Values.*.configFilesYAML.*.content.*`\n'
  printf -- '- `Values.*.envYAML.*`\n'
  printf -- '- `Values.*.extraFields` (any level)\n'
  printf -- '- `Values.apps-service-accounts.<app>.roles|clusterRoles.<name>.rules.<rule>.apiGroups|resources|verbs|resourceNames|nonResourceURLs`\n'
  printf -- '- `Values.apps-service-accounts.<app>.roles|clusterRoles.<name>.binding.subjects`\n'
  printf -- '- `Values.*.containers.<name>.sharedEnvConfigMaps`\n'
  printf -- '- `Values.*.initContainers.<name>.sharedEnvConfigMaps`\n'
  printf -- '- `Values.*.containers.<name>.sharedEnvSecrets`\n'
  printf -- '- `Values.*.initContainers.<name>.sharedEnvSecrets`\n'
  printf -- '- additional built-in list fields only when `global.validation.allowNativeListsInBuiltInListFields=true`\n\n'

  printf '## Strict Mode Checks\n'
  printf -- '- `global.validation.strict=true` enables:\n'
  printf -- '  - unknown top-level `apps-*` group detection (`E_STRICT_UNKNOWN_GROUP`)\n'
  printf -- '  - unknown key detection in strict-checked scopes (`E_STRICT_UNKNOWN_KEY`)\n\n'

  printf '## Tpl Delimiter Validation\n'
  printf -- '- `global.validation.validateTplDelimiters` controls `E_TPL_DELIMITERS`/`E_TPL_BRACES` checks in tpl-like strings\n'
  printf -- '- default behavior is backward-compatible (disabled unless enabled explicitly)\n\n'

  printf '## Library Entry Point\n'
  printf -- '- Consumer chart must call: `{{ include "apps-utils.init-library" $ }}`\n\n'

  printf '## Regeneration\n'
  printf 'Run:\n'
  printf '```bash\n'
  printf 'bash scripts/generate-capabilities-prompt.sh\n'
  printf '```\n'
} > "$OUT"

echo "Generated $OUT"
