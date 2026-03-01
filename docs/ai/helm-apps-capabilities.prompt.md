# helm-apps Capability Catalog (Prompt Input)

Generated from code on 2026-02-27 14:38:51Z.

Sources:
- `/Users/zol/src/helm-apps/tests/.helm/values.schema.json`
- `/Users/zol/src/helm-apps/charts/helm-apps/templates`
- `/Users/zol/src/helm-apps/AGENTS.md`

Use this file as machine-oriented context for AI prompts about helm-apps values format and rendering behavior.

## Top-Level Values Sections
- `apps-certificates`
- `apps-configmaps`
- `apps-cronjobs`
- `apps-custom-prometheus-rules`
- `apps-dex-authenticators`
- `apps-dex-clients`
- `apps-grafana-dashboards`
- `apps-infra`
- `apps-ingresses`
- `apps-jobs`
- `apps-k8s-manifests`
- `apps-kafka-strimzi`
- `apps-limit-range`
- `apps-network-policies`
- `apps-pvcs`
- `apps-secrets`
- `apps-service-accounts`
- `apps-services`
- `apps-stateful`
- `apps-stateless`
- `global`
- `helm-apps`
- `werf`

## Top-Level Section -> Schema Definition
- `global` -> `#/$defs/global`
- `apps-configmaps` -> `#/$defs/appMap`
- `apps-cronjobs` -> `#/$defs/appMap`
- `apps-ingresses` -> `#/$defs/appMap`
- `apps-jobs` -> `#/$defs/appMap`
- `apps-secrets` -> `#/$defs/appMap`
- `apps-stateful` -> `#/$defs/appMap`
- `apps-stateless` -> `#/$defs/appMap`
- `apps-custom-prometheus-rules` -> `#/$defs/appMap`
- `apps-limit-range` -> `#/$defs/appMap`
- `apps-pvcs` -> `#/$defs/appMap`
- `apps-certificates` -> `#/$defs/appMap`
- `apps-kafka-strimzi` -> `#/$defs/appMap`
- `apps-dex-authenticators` -> `#/$defs/appMap`
- `apps-dex-clients` -> `#/$defs/appMap`
- `apps-grafana-dashboards` -> `#/$defs/appMap`
- `apps-services` -> `#/$defs/appMap`
- `apps-service-accounts` -> `#/$defs/appMap`
- `apps-network-policies` -> `#/$defs/networkPolicyAppMap`
- `apps-k8s-manifests` -> `#/$defs/appMap`
- `apps-infra` -> `#/$defs/appsInfra`
- `werf` -> `object`
- `helm-apps` -> `object`

## Global Keys
- `global._includes`
- `global.ci_url`
- `global.deploy`
- `global.env`
- `global.labels`
- `global.releases`
- `global.validation`

## Global Validation Flags
- `global.validation.allowNativeListsInBuiltInListFields`
- `global.validation.strict`
- `global.validation.validateTplDelimiters`

## Common App Model Keys (`$defs.app`)
- `__AppType__`
- `_include`
- `accessModes`
- `activeDeadlineSeconds`
- `affinity`
- `allowedGroups`
- `alwaysRestart`
- `annotations`
- `applicationDomain`
- `applicationIngressCertificateSecretName`
- `applicationIngressClassName`
- `backoffLimit`
- `binaryData`
- `class`
- `clusterIssuer`
- `concurrencyPolicy`
- `containers`
- `data`
- `deckhouseMetrics`
- `dexAuth`
- `enabled`
- `entityOperator`
- `envVars`
- `exporter`
- `failedJobsHistoryLimit`
- `folder`
- `groups`
- `horizontalPodAutoscaler`
- `host`
- `hosts`
- `imagePullSecrets`
- `ingressClassName`
- `initContainers`
- `kafka`
- `labels`
- `limits`
- `name`
- `nodeSelector`
- `paths`
- `podDisruptionBudget`
- `priorityClassName`
- `randomName`
- `redirectURIs`
- `resources`
- `restartPolicy`
- `schedule`
- `selector`
- `sendAuthorizationHeader`
- `service`
- `serviceAccount`
- `startingDeadlineSeconds`
- `storageClassName`
- `successfulJobsHistoryLimit`
- `tls`
- `tolerations`
- `topics`
- `topologySpreadConstraints`
- `type`
- `versionKey`
- `verticalPodAutoscaler`
- `volumes`
- `werfWeight`
- `zookeeper`

## Specialized Model Keys
### container
- `args`
- `command`
- `configFiles`
- `configFilesYAML`
- `enabled`
- `env`
- `envFrom`
- `envVars`
- `envYAML`
- `fromSecretsEnvVars`
- `image`
- `lifecycle`
- `livenessProbe`
- `name`
- `persistantVolumes`
- `ports`
- `readinessProbe`
- `resources`
- `secretConfigFiles`
- `secretEnvVars`
- `securityContext`
- `sharedEnvConfigMaps`
- `sharedEnvSecrets`
- `startupProbe`
- `volumeMounts`
- `volumes`

### image
- `generateSignatureBasedTag`
- `name`
- `staticTag`

### resources
- `limits`
- `requests`

### resourcesGroup
- `ephemeralStorageMb`
- `mcpu`
- `memoryMb`

### configFile
- `content`
- `defaultMode`
- `mountPath`
- `name`

### service
- `annotations`
- `enabled`
- `headless`
- `name`
- `ports`
- `selector`

### serviceAccount
- `clusterRole`
- `enabled`
- `name`

### hpa
- `behavior`
- `customMetricResources`
- `enabled`
- `maxReplicas`
- `metrics`
- `minReplicas`

### vpa
- `enabled`
- `resourcePolicy`
- `updateMode`

### pdb
- `enabled`
- `maxUnavailable`
- `minAvailable`

### tls
- `enabled`
- `secret_name`

### networkPolicyApp
- `_include`
- `annotations`
- `apiVersion`
- `egress`
- `egressDeny`
- `enabled`
- `endpointSelector`
- `extraSpec`
- `ingress`
- `ingressDeny`
- `kind`
- `labels`
- `name`
- `podSelector`
- `policyTypes`
- `selector`
- `spec`
- `type`
- `types`

### appsInfra
- `node-groups`
- `node-users`

### appsInfraNodeUser
- `annotations`
- `enabled`
- `extraGroups`
- `isSudoer`
- `labels`
- `nodeGroups`
- `passwordHash`
- `sshPublicKey`
- `sshPublicKeys`
- `uid`

### customGroupVars
- `_include`
- `_preRenderAppHook`
- `_preRenderGroupHook`
- `enabled`
- `type`

## Built-In Renderers (`*.render`)
- `apps-certificates.render`
- `apps-configmaps.render`
- `apps-cronjobs.render`
- `apps-custom-prometheus-rules.render`
- `apps-deckhouse-metrics.render`
- `apps-dex-authenticators.render`
- `apps-dex-clients.render`
- `apps-grafana-dashboards.render`
- `apps-ingresses.render`
- `apps-jobs.render`
- `apps-k8s-manifests.render`
- `apps-kafka-strimzi.render`
- `apps-limit-range.render`
- `apps-network-policies.render`
- `apps-pvcs.render`
- `apps-secrets.render`
- `apps-service-accounts.render`
- `apps-services.render`
- `apps-stateful.render`
- `apps-stateless.render`

## All Defined Templates (Full Inventory)
- `_apps-utils.initCurrentGroupVars`
- `_fl.getValueRegex`
- `_fl.make_includes_from`
- `_getMapKeyValue`
- `apps-adopt-utils.adopt-specs`
- `apps-api-versions.cronJob`
- `apps-api-versions.horizontalPodAutoscaler`
- `apps-api-versions.podDisruptionBudget`
- `apps-api-versions.verticalPodAutoscaler`
- `apps-certificates`
- `apps-certificates.render`
- `apps-check-password`
- `apps-compat.assertNoUnexpectedLists`
- `apps-compat.enforceAllowedKeys`
- `apps-compat.normalizeServiceSpec`
- `apps-compat.normalizeStatefulSetSpec`
- `apps-compat.renderRaw`
- `apps-compat.renderRawResolved`
- `apps-compat.resolveRawJson`
- `apps-compat.validateTopLevelStrict`
- `apps-components._generate-config-checksum`
- `apps-components._service`
- `apps-components.cerificate`
- `apps-components.generate-config-checksum`
- `apps-components.generateConfigMapsAndSecrets`
- `apps-components.horizontalPodAutoscaler`
- `apps-components.podDisruptionBudget`
- `apps-components.service`
- `apps-components.verticalPodAutoscaler`
- `apps-configmaps`
- `apps-configmaps.render`
- `apps-cronjobs`
- `apps-cronjobs.render`
- `apps-custom-prometheus-rules`
- `apps-custom-prometheus-rules.render`
- `apps-deckhouse-metrics`
- `apps-deckhouse-metrics.render`
- `apps-deckhouse.metrics`
- `apps-default-values`
- `apps-dex-authenticators`
- `apps-dex-authenticators.render`
- `apps-dex-clients`
- `apps-dex-clients.render`
- `apps-grafana-dashboards`
- `apps-grafana-dashboards.render`
- `apps-helpers._generateConfigYAML`
- `apps-helpers._generateConfigYAML.clean`
- `apps-helpers.activateContainerForDefault`
- `apps-helpers.generateAnnotations`
- `apps-helpers.generateConfigYAML`
- `apps-helpers.generateContainers`
- `apps-helpers.generateEnvYAML`
- `apps-helpers.generateHPAMetrics`
- `apps-helpers.generateSharedEnvConfigMapsEnvFrom`
- `apps-helpers.generateSharedEnvSecretsEnvFrom`
- `apps-helpers.generateVolumeMounts`
- `apps-helpers.generateVolumes`
- `apps-helpers.genereteContainersEnv`
- `apps-helpers.genereteContainersEnvFrom`
- `apps-helpers.jobTemplate`
- `apps-helpers.metadataGenerator`
- `apps-helpers.podTemplate`
- `apps-infra`
- `apps-infra.node-groups`
- `apps-infra.node-users`
- `apps-ingresses`
- `apps-ingresses.render`
- `apps-jobs`
- `apps-jobs.render`
- `apps-k8s-manifests`
- `apps-k8s-manifests.emitTopField`
- `apps-k8s-manifests.render`
- `apps-k8s-manifests.renderRawResolved`
- `apps-k8s-manifests.resolveMapJson`
- `apps-k8s-manifests.resolveRawJson`
- `apps-kafka-strimzi`
- `apps-kafka-strimzi.render`
- `apps-limit-range`
- `apps-limit-range.render`
- `apps-network-policies`
- `apps-network-policies.render`
- `apps-pvcs`
- `apps-pvcs.render`
- `apps-release.prepareApp`
- `apps-secrets`
- `apps-secrets.render`
- `apps-service-accounts`
- `apps-service-accounts._metadataNamespaced`
- `apps-service-accounts._namespace`
- `apps-service-accounts._rbacObjectName`
- `apps-service-accounts._renderRoleAndBinding`
- `apps-service-accounts._renderRuleItem`
- `apps-service-accounts._renderRulesList`
- `apps-service-accounts.render`
- `apps-services`
- `apps-services.render`
- `apps-specs.containers.volumes`
- `apps-specs.selector`
- `apps-specs.serviceName`
- `apps-specs.volumeClaimTemplates`
- `apps-stateful`
- `apps-stateful.render`
- `apps-stateless`
- `apps-stateless.render`
- `apps-system.serviceAccount`
- `apps-utils._includesFromFiles`
- `apps-utils.currentPath`
- `apps-utils.enterScope`
- `apps-utils.error`
- `apps-utils.findApps`
- `apps-utils.generateSpecs`
- `apps-utils.includesFromFiles`
- `apps-utils.init-library`
- `apps-utils.leaveScope`
- `apps-utils.preRenderHooks`
- `apps-utils.printPath`
- `apps-utils.renderApps`
- `apps-utils.requiredValue`
- `apps-utils.tpl`
- `apps-version.getLibraryVersion`
- `apps.generateConfigMapData`
- `apps.generateConfigMapEnvVars`
- `apps.generateContainerEnvVars`
- `apps.generateSecretEnvVars`
- `apps.value`
- `fl.Result`
- `fl._concatLists`
- `fl._getJoinedIncludesInJson`
- `fl._recursiveMapsMerge`
- `fl._recursiveMergeAndExpandIncludes`
- `fl._renderValue`
- `fl._validateTplValue`
- `fl.currentEnv`
- `fl.expandIncludesInValues`
- `fl.formatStringAsDNSLabel`
- `fl.formatStringAsDNSSubdomain`
- `fl.generateConfigMapData`
- `fl.generateConfigMapEnvVars`
- `fl.generateContainerEnvVars`
- `fl.generateContainerFromSecretsEnvVars`
- `fl.generateContainerImageQuoted`
- `fl.generateContainerResources`
- `fl.generateLabels`
- `fl.generateSecretData`
- `fl.generateSecretEnvVars`
- `fl.generateSelectorLabels`
- `fl.isFalse`
- `fl.isTrue`
- `fl.percentage`
- `fl.tplDelimitersValidationEnabled`
- `fl.value`
- `fl.valueQuoted`
- `fl.valueSingleQuoted`
- `kafka-topics`

## Native YAML List Policy (From `apps-compat.assertNoUnexpectedLists`)
Native lists are generally forbidden, except allowed paths/fields detected in code:
- `_include`
- `_include_files`
- `Values.global._includes.*`
- `Values.apps-kafka-strimzi.*.kafka.brokers.hosts.*`
- `Values.apps-kafka-strimzi.*.kafka.ui.dex.allowedGroups.*`
- `Values.*.configFilesYAML.*.content.*`
- `Values.*.envYAML.*`
- `Values.*.extraFields` (any level)
- `Values.apps-service-accounts.<app>.roles|clusterRoles.<name>.rules.<rule>.apiGroups|resources|verbs|resourceNames|nonResourceURLs`
- `Values.apps-service-accounts.<app>.roles|clusterRoles.<name>.binding.subjects`
- `Values.*.containers.<name>.sharedEnvConfigMaps`
- `Values.*.initContainers.<name>.sharedEnvConfigMaps`
- `Values.*.containers.<name>.sharedEnvSecrets`
- `Values.*.initContainers.<name>.sharedEnvSecrets`
- additional built-in list fields only when `global.validation.allowNativeListsInBuiltInListFields=true`

## Strict Mode Checks
- `global.validation.strict=true` enables:
  - unknown top-level `apps-*` group detection (`E_STRICT_UNKNOWN_GROUP`)
  - unknown key detection in strict-checked scopes (`E_STRICT_UNKNOWN_KEY`)

## Tpl Delimiter Validation
- `global.validation.validateTplDelimiters` controls `E_TPL_DELIMITERS`/`E_TPL_BRACES` checks in tpl-like strings
- default behavior is backward-compatible (disabled unless enabled explicitly)

## Library Entry Point
- Consumer chart must call: `{{ include "apps-utils.init-library" $ }}`

## Regeneration
Run:
```bash
bash scripts/generate-capabilities-prompt.sh
```
