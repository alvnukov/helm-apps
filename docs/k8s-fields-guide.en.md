# Kubernetes Fields in Helm Apps (Developer Guide)

This guide explains Kubernetes fields commonly used through helm-apps `values.yaml`.
Focus: what each field changes in runtime behavior and when to touch it.

## image
<a id="image"></a>

What it is: container image settings (`name`, `staticTag`).

Use it when:
- releasing a new app version;
- rolling back to a previous version;
- switching registry/image.

## command and args
<a id="command-and-args"></a>

What it is: container startup command and arguments.

Use it when:
- you need to override container entrypoint;
- you need to pass runtime flags to the app.

## probes (liveness, readiness, startup)
<a id="probes-liveness-readiness-startup"></a>

What they are:
- `readinessProbe`: can this pod receive traffic now;
- `livenessProbe`: should Kubernetes restart this container;
- `startupProbe`: protects slow-starting apps.

Practical defaults:
- start with `readinessProbe`;
- add `livenessProbe` only when health endpoint is stable;
- use `startupProbe` for long boot times.

## securityContext
<a id="security-context"></a>

What it is: process security options (`runAsNonRoot`, `runAsUser`, capabilities, readOnlyRootFilesystem).

Why it matters:
- reduces privilege escalation risk;
- helps pass cluster security policies.

## affinity, tolerations, nodeSelector
<a id="affinity-tolerations-nodeselector"></a>

What they are:
- `nodeSelector`: simple node matching by labels;
- `affinity`: advanced placement rules;
- `tolerations`: allow scheduling to tainted nodes.

Use cases:
- isolate prod workloads from system nodes;
- spread replicas across hosts/zones;
- run on specialized node pools.

## volumes and volumeMounts
<a id="volumes-and-volumemounts"></a>

What they are:
- `volumes`: data sources (emptyDir, secret, configMap, pvc, etc.);
- `volumeMounts`: mount locations inside container filesystem.

Common scenarios:
- config files under `/etc/...`;
- certs/secrets as files;
- temp data via `emptyDir`.

## lifecycle hooks
<a id="lifecycle-hooks"></a>

What they are: `postStart` and `preStop` hooks.

When to use:
- graceful shutdown logic (`preStop`);
- one-time startup init (`postStart`).

## envFrom
<a id="envfrom"></a>

What it is: bulk env import from ConfigMap/Secret.

Important:
- convenient for shared env bundles;
- can create key conflicts, so validate final env set.

## ports
<a id="ports"></a>

What it is: container/service port declarations.

Use it when:
- app listening port changes;
- adding extra protocol/port;
- aligning service port with container port.

## How this relates to helm-apps helpers

- `envVars` / `secretEnvVars` / `sharedEnv*` are helper layers over Kubernetes env mechanisms.
- `configFiles*` helpers generate ConfigMap/Secret + mounts.
- `resources` helper controls CPU/Memory requests and limits.

See also:
- [reference-values.md](reference-values.md)
- [cookbook.md](cookbook.md)
