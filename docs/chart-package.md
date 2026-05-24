# Chart Package Offline Docs

The `helm-apps` chart package includes documentation for air-gapped use without keeping a second copy under the chart source tree.

Packaging contract:

- `charts/helm-apps/AGENTS.md` is a symlink to `docs/ai/helm-apps-offline-agents.md`.
- `charts/helm-apps/docs` is a symlink to the repository `docs/` directory.
- `helm package charts/helm-apps` follows these symlinks and includes their contents in the chart archive.

Source of truth:

- Update documentation only in `docs/`.
- Do not edit generated or copied chart-local documentation copies.
- Do not replace chart symlinks with duplicated files.

Recommended offline reading order inside an unpacked chart package:

1. `AGENTS.md` - LLM operating contract and syntax guardrails.
2. `docs/ai/helm-apps-capabilities.prompt.md` - compact machine-readable syntax catalog.
3. `docs/reference-values.md` - complete values reference and validation flags.
4. `docs/decision-guide.md` - choosing the right `apps-*` group and value shape.
5. `docs/quickstart.md`, `docs/cookbook.md`, and `docs/faq.md` - examples and common mistakes.
6. `templates/_apps-*.tpl` - final source for renderer behavior when docs are ambiguous.

If docs and templates disagree, trust rendered behavior from templates, then fix the docs in `docs/`.
