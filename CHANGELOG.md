# Changelog

All notable changes to the `helm-apps` library are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Automated GitHub Release notes generation in `release.yml`.

## [1.6.7] - 2026-02-17

### Added
- Strict YAML stream validation script for rendered outputs and snapshots: `scripts/validate-yaml-stream.rb`.
- Duplicate-key detection in YAML validation to catch missing document separators and merged-doc regressions early.

### Changed
- `apps-infra` rendering now uses shared path/header printing (`apps-utils.printPath`) for `NodeUser` and `NodeGroup` resources.
- CI, release workflow, and local CI now validate YAML syntax and stream integrity before snapshot/structure comparisons.
- Contract checks now validate all generated YAML streams (`production`, `dev`, `strict`, version matrix, internal-like flow).
- Contracts snapshot comparison is now normalized to ignore `helm-apps/version`, so checks are stable across patch releases.

### Fixed
- Invalid contracts snapshot structure caused by missing/incorrect document separation in `tests/contracts/test_render.snapshot.yaml`.

## [1.6.6] - 2026-02-16

### Added
- Container-level shared ConfigMap references via `sharedEnvConfigMaps` list (`envFrom.configMapRef`).
- Contract examples for reusable `apps-configmaps` + container-level `envFrom` wiring.

### Changed
- Extended values schema with `sharedEnvConfigMaps` validation (string names / env-map of strings).
- Native-list guard updated to allow `*.containers.*.sharedEnvConfigMaps` and `*.initContainers.*.sharedEnvConfigMaps`.
- Documentation expanded with detailed `sharedEnvConfigMaps` contract, precedence, and examples.

## [1.6.5] - 2026-02-16

### Changed
- Tightened `sharedEnvSecrets` schema to accept only string secret names (or env-map of strings).
- Restricted native-list allowance for `sharedEnvSecrets` to container paths only:
  - `*.containers.*.sharedEnvSecrets`
  - `*.initContainers.*.sharedEnvSecrets`

## [1.6.4] - 2026-02-16

### Added
- Shared environment secret references for containers via `sharedEnvSecrets` list.
- Contract examples for reusable `apps-secrets` + container-level `envFrom` wiring.

### Changed
- Validation schema updated to support `containers.*.sharedEnvSecrets` as a native list.
- Native-list guard updated to allow `*.sharedEnvSecrets` while preserving strict list checks elsewhere.
- CI and local CI checks updated for shared env secret rendering and wiring.

## [1.6.3] - 2026-02-16

### Added
- Kubernetes compatibility matrix checks in CI for multiple API levels (legacy and current).
- `kind` + server-side dry-run validation job with compatibility CRDs.
- Contracts coverage for all built-in `apps-*` entities.
- Property-based fuzz checks for contracts (`scripts/fuzz-contracts.sh`).
- Entity coverage gate script (`scripts/check-entity-coverage.sh`) and CI step to enforce coverage.
- Stability and reliability documentation:
  - `docs/stability.md`
  - expanded reliability section in `README.md`.

### Changed
- README branding improvements (icon + badges).
- Local CI helper (`scripts/ci-local.sh`) updated to include coverage and fuzz checks.

## [1.6.0] - 2026-02-16

### Added
- New release matrix mode via `global.release`:
  - `enabled`, `current`, `autoEnableApps`, `versions`.
- Added app-level `releaseKey` to map an app to release matrix keys.
- Automatic release annotations in rendered manifests:
  - `helm-apps/release`
  - `helm-apps/app-version`
- Added release-mode contract checks in CI.

### Changed
- If `image.staticTag` is not set, image tag can be resolved from `CurrentAppVersion`.
- Extended `tests/.helm/values.schema.json` with `global.release` and `releaseKey`.
- Updated docs (`README.md`, `docs/reference-values.md`, `docs/parameter-index.md`) with release mode examples.

## [1.5.0] - 2026-02-16

### Added
- Added `apps-network-policies` entity with `type`-based implementation selection:
  - `kubernetes` -> `networking.k8s.io/v1`, `NetworkPolicy`
  - `cilium` -> `cilium.io/v2`, `CiliumNetworkPolicy`
  - `calico` -> `projectcalico.org/v3`, `NetworkPolicy`
- Added contract tests and CI checks for multiple NetworkPolicy implementations.
- Added opt-in strict validation:
  - unknown keys in `apps-network-policies` fail;
  - unknown top-level `apps-*` groups fail unless declared via `__GroupVars__.type`.

### Changed
- Expanded user documentation and parameter navigation.
- Added values schema validation in CI.

## [1.4.0] - 2026-02-16

### Added
- Added passthrough fields for Kubernetes entities:
  - workload/container `extra*` fields for new or uncommon API fields without library changes.
- Added compatibility tests for multiple Kubernetes API versions.

### Changed
- Improved compatibility across legacy and modern Kubernetes versions.

## [1.3.2] - 2024-02-13

### Fixed
- Fixed handling of `null` variables in ConfigMap YAML.

## [1.3.1] - 2024-02-12

### Fixed
- Additional fixes for `null` variable handling in ConfigMap YAML.

## [1.3.0] - 2022-03-22

### Added
- Helm 3 compatibility.

## [1.2.9] - 2021-12-07

### Fixed
- Error handling for include blocks loaded from files.

## [1.2.8] - 2021-07-13

### Fixed
- Support for `tpl` in include file names.

## [1.2.7] - 2021-06-03

### Fixed
- Correct merge behavior with `_default`.

## [1.2.6] - 2021-05-21

### Fixed
- Added `_include_files` support.
