# Flant Upstream PR Draft

This file prepares an upstream PR package for merging key `helm-apps` improvements into Flant library flow.

## 1. Recommended PR Scope

To keep review safe and predictable, split into 3 PRs:

1. **Compatibility + validation safety**
2. **NetworkPolicy entity (CNI-aware)**
3. **Release matrix mode + docs/contracts**

This avoids one huge diff and keeps rollback simple.

## 2. Commit Candidates (from current repo)

### PR-1: Compatibility + validation safety

- `d0c2d21` feat(compat): kubernetes version-aware api/spec compatibility checks
- `9c28e30` fix(validation): fail on unexpected native lists and show exact values path
- `1128e61` fix(validation): allow native lists in approved template-driven paths
- `b4b50de` fix(stability): keep full list validation traversal and safe hasKey checks
- `18e1a6f` feat(strict): opt-in unknown-key validation for network policies
- `be5bf5e` feat(strict): unknown top-level apps groups validation with custom-group allowance

### PR-2: NetworkPolicy entity

- `48e8b08` feat(network-policy): add cni-aware network policy entity with type-based rendering

### PR-3: Release matrix mode

- `f4db8a2` Add release matrix mode with image tag fallback, schema/docs/contracts, and CI coverage
- `2915860` fix: improve schema compatibility and add internal-like test coverage
- `c3cae63` fix(docs): clarify optional releaseKey and app name fallback
- `614e0a0` fix(docs): clarify release defaults, fallbacks, and custom group type behavior
- `01b5164` fix(ci): add internal-like contract scenario for release/deploy flow

## 3. PR Text Template (English)

Use this in GitHub PR description:

```md
## What

This PR introduces compatibility and validation improvements for the helm-apps library:

- Kubernetes-version-aware API rendering for resources with changed API versions.
- Safer values validation:
  - fail on unexpected native YAML lists with exact values path in error;
  - preserve allowed list-based subtrees used by template-driven fields;
  - opt-in strict validation for selected entities/groups.
- Extended contract checks for backward/forward compatibility.

## Why

The main goal is to improve deployment reliability across mixed Kubernetes versions and reduce silent misconfiguration risks in large values trees.

## Backward compatibility

- Behavior remains backward-compatible by default.
- Strict unknown-key checks are opt-in via `global.validation.strict`.
- Existing templates using string-based Kubernetes blocks continue to work unchanged.

## Validation

Locally validated with:

- `werf helm lint tests/.helm --values tests/.helm/values.yaml`
- contract templates checks (`tests/contracts`)
- Kubernetes compatibility matrix (render + kubeconform):
  - 1.19.16
  - 1.20.15
  - 1.23.17
  - 1.29.0

## Notes for reviewers

- Validation failure messages now include exact values path for faster troubleshooting.
- Custom top-level groups remain supported through `__GroupVars__.type`.
```

## 4. Upstream Checklist

- [ ] Confirm target Flant repo and base branch.
- [ ] Create feature branch in upstream fork.
- [ ] Cherry-pick commits for selected PR scope.
- [ ] Resolve path conflicts in templates/schema/tests.
- [ ] Run local checks:
  - [ ] `bash scripts/ci-local.sh --skip-snapshot`
  - [ ] matrix render checks for 1.19/1.20/1.23/1.29
- [ ] Push branch and open PR with template text.
- [ ] Attach rendered diff snippets for risky API-switch parts.

## 5. Suggested Branch Names

- `flant/compat-validation-safety`
- `flant/network-policy-cni-aware`
- `flant/release-matrix-mode`

