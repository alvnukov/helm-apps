# happ-rs

Rust implementation of `happ` focused on import/inspect/diff/query workflows.

## Query commands

`happ jq` and `happ yq` now differ by query language style only.

- `happ jq`: jq-like syntax
- `happ yq`: yq-like syntax
- both commands accept **JSON and YAML** input (auto-detected)

### Examples

```bash
# jq syntax over YAML input
happ jq --query '.apps[] | .name' --input values.yaml
```

```bash
# yq syntax over JSON input
happ yq --query '.apps[] | .name' --input values.json
```

```bash
# stdin also supports both formats
cat values.yaml | happ jq --query '.global.env' --input -
cat values.json | happ yq --query '.global.env' --input -
```

Output options:

- `--compact`
- `--raw-output` (prints raw string values without JSON quotes)

## Shell completion

`happ` can generate completion scripts for:

- `bash`
- `zsh`
- `fish`
- `powershell`
- `elvish`

Examples:

```bash
# print to stdout
happ completion --shell zsh
```

```bash
# write to file
happ completion --shell bash --output /tmp/happ.bash
```

```bash
# web mode for tests/CI without opening browser
happ --web --web-open-browser=false
```

## Parity Matrix (CLI contracts)

Core CLI behavior is pinned by integration parity tests.

- test file: `tests/parity_cli.rs`
- fixtures: `tests/parity/fixtures/*`
- covered contracts:
  - `help`
  - `validate`
  - `jq`
  - `yq`
  - `dyff`
  - `manifests`
  - `compose`
  - `completion`
  - embedded `charts/helm-apps` asset generation

Run locally:

```bash
cargo test --test parity_cli
```
