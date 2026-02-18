# Changelog

All notable changes to the `helm-apps` library are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Automated GitHub Release notes generation in `release.yml`.

## [1.7.0] - 2026-02-18

### Changed
- Release matrix contract moved to `global.deploy` + `global.releases`.
- Release selection now resolves by `global.env` through `global.deploy.release` (env-map/string).
- App release key renamed to `versionKey`.
- Documentation, schema, examples, and contracts aligned with the new release contract.

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
