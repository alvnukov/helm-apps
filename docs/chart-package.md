# Chart Package Contents

The `helm-apps` chart archive contains runtime templates, `Chart.yaml` and `values.yaml`. Since version `1.8.13`, it intentionally excludes repository documentation and `AGENTS.md`; removing those files reduced the archive size by about 95%.

Packaging contract:

- Do not add `charts/helm-apps/docs` or `charts/helm-apps/AGENTS.md` symlinks.
- `helm package charts/helm-apps` packages the runtime chart without the repository `docs/` tree.
- Documentation is maintained in the repository `docs/` directory.

For air-gapped work, transfer the documentation separately alongside the chart archive. Recommended reading order in that separate documentation copy:

1. [Offline agent guide](ai/helm-apps-offline-agents.md) — syntax guardrails and verification steps.
2. [Capability catalog](ai/helm-apps-capabilities.prompt.md) — machine-readable syntax summary.
3. [Values reference](reference-values.md) — parameter semantics and validation flags.
4. [Decision guide](decision-guide.md) — choosing the resource group and value shape.
5. [Quick start](quickstart.md), [cookbook](cookbook.md) and [FAQ](faq.md) — examples and common mistakes.
6. The chart's `templates/_apps-*.tpl` files — actual renderer behavior.

When schema, documentation and rendered output disagree, report the mismatch and verify it before changing consumer values.
