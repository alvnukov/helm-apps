# helm-apps VS Code Extension

VS Code tooling for `values.yaml` in `helm-apps` format.

## Features

- Auto-attach bundled `helm-apps` JSON Schema to `values*.yaml`.
- Command: `helm-apps: Configure YAML schema`.
- Command: `helm-apps: Validate current values.yaml` (runs built-in helm-apps diagnostics).
- Explorer view: `helm-apps Values Structure` for visual tree of current YAML keys.
  - Click any node to jump to its line in the file.
- Activity Bar section: `helm-apps` with `Quick Actions`.
  - One-click actions for scaffold, preview, validation, settings, and paste-as-helm-apps.
- Refactor command: `helm-apps: Extract app child to global include`.
  - Cursor on direct app child key (e.g. `labels`, `resources`) -> moves it into `global._includes.<name>` and adds `_include`.
- Refactor command: `helm-apps: Safe rename app key`.
  - Renames `apps-*.<app>` key and updates `global.releases.*.<app>` keys.
- Preview command: `helm-apps: Preview resolved entity (with includes)`.
  - Opens side panel and shows `apps-*.<app>` with options:
    - env selection (including discovered regex keys),
    - toggle `apply includes`,
    - toggle `resolve env maps`.
  - Preview loader applies `_include_from_file` and `_include_files`.
- Navigation command: `helm-apps: Go to include definition`.
  - Works on `_include` list items in YAML.
  - Jumps to `global._includes.<name>` in current file or to include-file definition.
- Symbol navigation/refactor:
  - `Find Usages` for include/app symbols (also command: `helm-apps: Find symbol usages`).
  - `Rename Symbol` for include/app symbols updates linked occurrences in current file.
  - `Go to Definition` for app keys referenced from `global.releases.*`.
- Dependency graph command: `helm-apps: Open dependency graph`.
  - Visualizes `apps -> _include`, `global._includes`, and include-file references.
- Clipboard import command: `helm-apps: Paste as helm-apps`.
  - Reads Kubernetes manifests from clipboard.
  - Converts via `happ manifests --import-strategy helpers-experimental`.
  - Inserts converted helm-apps values into current editor (replaces selection if any).
- Bootstrap command: `helm-apps: Create Starter helm-apps Chart`.
  - Creates a new Helm chart scaffold from empty folder/workspace.
  - Adds `templates/init-helm-apps-library.yaml` with required `apps-utils.init-library`.
  - Copies bundled `helm-apps` library into `<chart>/charts/helm-apps` in unpacked form (offline, no internet).
- Outline / Document Symbols:
  - sections (`global`, groups), apps, include profiles, and app fields are exposed in editor Outline.
- Hover on `_include` item shows include code snippet.
  - Source can be local `global._includes`, include-file content, or resolved include body.
- Library settings command: `helm-apps: Open library settings`.
  - Opens visual settings panel with explained library options.
  - Applies selected toggles directly into `global.*` in active `values.yaml`.
- Library settings help command: `helm-apps: Generate library settings help`.
  - Generates Markdown help based on current setting states.
  - Also available from the settings panel (`Generate help` button).
- Smart editing:
  - Completion snippets for common app keys (`enabled`, `_include`, `resources`, `envVars`, `service`).
  - Quick fixes:
    - convert inline `_include: [a, b]` into multiline list,
    - add `enabled: true` into app block,
    - create missing include profile from `Unresolved include profile` diagnostic.
- Semantic diagnostics:
  - `Unresolved include profile` warning.
  - `Unused include profile` information diagnostic.

## Requirements

- VS Code YAML extension: `redhat.vscode-yaml`.
- `happ` binary available in `PATH` (or set `helm-apps.happPath`) for import/conversion commands.

## Settings

- `helm-apps.schemaFileMatch` - glob patterns for files that should use schema.
- `helm-apps.happPath` - path to `happ` binary.
- `helm-apps.disableYamlSchemaHover` - hides generic YAML schema hover blocks (like `Source: values.schema.json`) and keeps helm-apps contextual hover only.
