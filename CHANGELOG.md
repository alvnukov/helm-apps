# Changelog

All notable changes to the `helm-apps` library are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.8.10] - 2026-04-06

### Fixed
- Release-derived app version no longer overrides `initContainers` image resolution.
- `apps-jobs` and other built-in workloads now keep using `image.staticTag`, `Values.werf.image`, and `Values.global.werfReport.image` for `initContainers`, even when the parent app participates in release mode.

## [1.8.9] - 2026-03-19

### Fixed
- Tightened built-in release-mode enablement:
  - `global.deploy.enabled` is now strict; if the key is absent, built-in release logic stays disabled;
  - `annotateAllWithRelease` and release-derived image/app-version logic no longer activate when `enabled` is absent.
- Fixed empty env-resolved release versions:
  - a release entry that resolves to an empty value for current `global.env` is now treated as "app not in release";
  - such entries no longer auto-enable apps, set empty `helm-apps/app-version`, or render broken image refs like `name:`.

## [1.8.8] - 2026-03-18

### Fixed
- Split release-mode master switch from app auto-enable:
  - `global.deploy.enabled` now controls built-in release logic only;
  - `global.deploy.autoEnableApps` now controls auto-enable only;
  - if `global.deploy.autoEnableApps` is absent, `null` or `false`, apps are not auto-enabled.
- Fixed release-mode leakage when built-in release logic is disabled:
  - `global.deploy.enabled=false` now blocks release-derived image tags and release annotations consistently;
  - `annotateAllWithRelease` is ignored when built-in release logic is disabled.
- Expanded contracts and docs for the new release-mode split:
  - added checks for `autoEnableApps=false`, `autoEnableApps=null` and missing `autoEnableApps`;
  - documented the separate responsibilities of `global.deploy.enabled` and `global.deploy.autoEnableApps`.

## [1.8.7] - 2026-03-17

### Added
- Added a new last-resort image fallback `global.werfReport.image` for shared/nested charts importing image refs from `werf build report`.
- Added dedicated documentation for passing `werf build report` into charts with both `zq` and `jq` examples.

## [1.8.6] - 2026-03-13

### Fixed
- Restored pre-`1.8.0` list semantics for native built-in list fields:
  - native list items are rendered as raw user data;
  - no recursive `tpl`/env-resolution is applied inside list elements;
  - env selection is preserved only on the root list field value.
- Preserved old string DSL behavior for list blocks:
  - YAML block string list fields still render `tpl` before YAML parsing;
  - documented that native lists are not a full replacement for templated typed scalar fields.
- Expanded contracts around native list compatibility:
  - native list items with keys like `dev` / `_default` stay data;
  - string list path and native list path now have explicit regression coverage.
- Extended `global.validation.strict` to built-in workload sections (`apps-stateless`, `apps-stateful`, `apps-jobs`, `apps-cronjobs`) so typos in workload keys fail explicitly instead of being ignored.
- Added an opt-in guard for legacy `serviceAccount.clusterRole` inside workloads:
  - legacy path remains available for compatibility;
  - `global.validation.forbidLegacyServiceAccountClusterRole=true` now blocks it explicitly.
- Added fail-fast validation for built-in list fields such as `imagePullSecrets`, so wrong scalar values now fail during render with `E_LIST_FIELD` instead of surfacing later during deploy.

## [1.8.5] - 2026-03-13

### Added
- Automated GitHub Release notes generation in `release.yml`.
- Added experimental opt-in support for native YAML lists in built-in list fields via `global.validation.allowNativeListsInBuiltInListFields=true`.
- Added contract coverage for native lists in built-in list fields, including env-map branches.

### Changed
- Repository scope narrowed to Helm library only:
  - removed `cmd/happ-rs` and `extensions/helm-apps` from this repository;
  - CI and release workflows now validate and publish only `helm-apps` chart artifacts.
- Added cross-repo links in docs:
  - CLI moved to `alvnukov/happ`;
  - IDE extensions moved to `alvnukov/helm-apps-extensions`.

### Fixed
- Fixed internal template crashes (`nil is not a command`) in env-aware native list resolution for compatibility and generic manifests helpers.
- Fixed native `service.ports` rendering when `global.validation.allowNativeListsInBuiltInListFields=true`.

## [1.8.4] - 2026-03-07

### Fixed
- Clarified and locked `apps-network-policies` contract in docs for all built-in implementations:
  - `kubernetes` (`networking.k8s.io/v1`, `NetworkPolicy`);
  - `cilium` (`cilium.io/v2`, `CiliumNetworkPolicy`);
  - `calico` (`projectcalico.org/v3`, `NetworkPolicy`).
- Added direct parameter index navigation to dedicated `apps-network-policies` reference section.

## [1.8.3] - 2026-03-03

### Changed
- Removed deprecated Go implementation of `happ` (`cmd/happ`), Rust `happ-rs` is now the single CLI implementation in repository.
- Switched embedded library asset flow to build-time generation from актуальный `charts/helm-apps` via `build.rs` in `happ-rs`.

### Added
- Added CLI parity matrix integration tests (`tests/parity_cli.rs`) with fixed fixtures for:
  - `help`, `validate`, `jq`, `yq`, `dyff`,
  - `manifests`, `compose`, `completion`,
  - embedded chart generation contract.
- Added dedicated parity matrix steps in CI and release workflows.

## [1.7.6] - 2026-02-25

### Added
- Optional env label propagation for rendered entities:
  - `global.labels.addEnv=true` adds `app.kubernetes.io/environment=<current env>` to metadata labels.

### Fixed
- Added fail-fast validation for env-aware values rendering:
  - render now fails with `E_ENV_REQUIRED` when both `werf.env` and `global.env` are empty;
  - error message explicitly points to setting env at deploy/render stage.

## [1.7.5] - 2026-02-19

### Added
- Added `global.deploy.annotateAllWithRelease` option:
  - when enabled, `helm-apps/release` annotation is applied to all rendered resources for the current deploy.

### Fixed
- Fixed release annotation scoping:
  - by default, `helm-apps/release` is now applied only to apps found in `global.releases.<release>`;
  - release context (`CurrentReleaseVersion`/`CurrentAppVersion`) is reset per app to avoid annotation leakage.
- Expanded legacy and contracts coverage:
  - added explicit release-mode examples (`global.deploy`, `global.releases`, `versionKey`) in legacy values;
  - added shared env sources (`sharedEnvConfigMaps`, `sharedEnvSecrets`) coverage in legacy snapshot;
  - added `apps-routes-contract` and `calico` network policy examples to legacy snapshot;
  - updated contracts to assert both default and annotate-all release annotation behavior.

## [1.7.4] - 2026-02-19

### Fixed
- Fixed duplicate `workingDir` rendering in container specs by treating it strictly as a string field.
- Hardened `secretConfigFiles` contract:
  - required `content` or `name` per file entry;
  - added explicit render error for missing source (`E_CONFIG_FILE_SOURCE`);
  - aligned volumeMount/volume generation so mounts are created only when source is resolvable.
- Fixed config checksum calculation to include `secretConfigFiles` content (`secretConfigFiles` was previously skipped due to wrong key lookup).
- Fixed `VerticalPodAutoscaler.spec.targetRef.apiVersion` selection:
  - `CronJob` now uses kube-compatible cron API (`batch/v1` or `batch/v1beta1`);
  - `Job` now uses `batch/v1`;
  - workload defaults keep `apps/v1`.

### Changed
- Expanded contracts to cover:
  - `workingDir` rendering contract;
  - `secretConfigFiles` happy-path and negative validation case;
  - VPA targetRef apiVersion for `CronJob`/`Job` across kube versions.
- Normalized visual logical indentation across library template files for easier maintenance.

## [1.7.3] - 2026-02-19

### Fixed
- Fixed `_include` merge behavior for list values to keep lists atomic (except `_include` itself):
  - list values are inherited only when key is absent in the higher-priority layer;
  - existing list keys are no longer implicitly replaced by lower-priority include layers.
- Documented the explicit list contract for `configFilesYAML.content`:
  - env-map resolution order (`exact -> regex -> _default`);
  - no index-wise list merge;
  - whole-list replace semantics on explicit override.

## [1.7.2] - 2026-02-19

### Fixed
- Fixed env-regex resolution in `configFilesYAML` values for non-string types:
  - env-map values like numbers, booleans, and nested objects now correctly use regex matches (not only exact env or `_default`).
- Preserved existing behavior for string values and recursive cleanup of empty branches in generated YAML config maps.

## [1.7.1] - 2026-02-18

### Fixed
- Improved `fl.value` diagnostics for template strings:
  - added explicit errors for malformed delimiters (`E_TPL_DELIMITERS`);
  - added explicit errors for triple-brace usage (`E_TPL_BRACES`);
  - made environment resolution in `fl.value` safer when `global.env` is missing or non-string.
- Added structured, actionable error format across key render validations:
  - `[helm-apps:<CODE>] ... | path=... | hint=... | docs=...`.
- Kept compatibility for `_include_from_file` and `_include_files` when referenced file content is empty/missing, while preserving parse-error checks for non-empty invalid YAML.
- Expanded troubleshooting docs for `fl.value` template failures in FAQ and operations guide.

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
