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
