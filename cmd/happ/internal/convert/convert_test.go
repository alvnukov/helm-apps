package convert

import (
	"strings"
	"testing"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

func TestBuildValues_ExtractsIncludesAndSuffixesDuplicateKeys(t *testing.T) {
	cfg := config.Config{
		Env:             "prod",
		GroupName:       config.DefaultGroupName,
		GroupType:       config.DefaultGroupType,
		MinIncludeBytes: 1,
	}

	docs := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name": "same-name",
				"labels": map[string]any{
					"team": "platform",
				},
			},
			"data": map[string]any{"A": "1"},
		},
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name": "same-name",
				"labels": map[string]any{
					"team": "platform",
				},
			},
			"data": map[string]any{"A": "1"},
		},
	}

	values, err := BuildValues(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValues returned error: %v", err)
	}

	global := values["global"].(map[string]any)
	if global["env"] != "prod" {
		t.Fatalf("unexpected global.env: %#v", global["env"])
	}
	includes, ok := global["_includes"].(map[string]any)
	if !ok || len(includes) == 0 {
		t.Fatalf("expected extracted includes, got %#v", global["_includes"])
	}

	group := values[config.DefaultGroupName].(map[string]any)
	var apps []map[string]any
	for k, v := range group {
		if !strings.HasPrefix(k, "config-map-same-name") {
			continue
		}
		if m, ok := v.(map[string]any); ok {
			apps = append(apps, m)
		}
	}
	if len(apps) != 2 {
		t.Fatalf("expected duplicate app keys with suffix, got keys: %#v", group)
	}
	for _, app := range apps {
		if _, exists := app["metadata"]; exists {
			t.Fatalf("expected metadata moved to include")
		}
		inc, ok := app["_include"].([]any)
		if !ok || len(inc) == 0 {
			t.Fatalf("expected _include in app: %#v", app)
		}
	}
}

func TestBuildValues_StripsStatusByDefault(t *testing.T) {
	cfg := config.Config{
		Env:             "dev",
		GroupName:       config.DefaultGroupName,
		GroupType:       config.DefaultGroupType,
		MinIncludeBytes: 9999,
		IncludeStatus:   false,
	}
	doc := map[string]any{
		"apiVersion": "apps/v1",
		"kind":       "Deployment",
		"metadata":   map[string]any{"name": "demo"},
		"spec":       map[string]any{"replicas": 1},
		"status":     map[string]any{"readyReplicas": 1},
	}

	values, err := BuildValues(cfg, []map[string]any{doc})
	if err != nil {
		t.Fatalf("BuildValues returned error: %v", err)
	}
	group := values[config.DefaultGroupName].(map[string]any)
	app := group["deployment-demo"].(map[string]any)
	if _, ok := app["status"]; ok {
		t.Fatalf("status should not exist as direct field: %#v", app)
	}
	if ef, _ := app["extraFields"].(string); strings.Contains(ef, "status:") {
		t.Fatalf("status should be excluded by default: %q", ef)
	}
}

func TestBuildValues_FiltersHelmManagedMetadataLabels(t *testing.T) {
	cfg := config.Config{
		Env:             "dev",
		GroupName:       config.DefaultGroupName,
		GroupType:       config.DefaultGroupType,
		MinIncludeBytes: 9999,
	}
	doc := map[string]any{
		"apiVersion": "v1",
		"kind":       "ConfigMap",
		"metadata": map[string]any{
			"name": "demo",
			"labels": map[string]any{
				"helm.sh/chart":                "nginx-1.2.3",
				"app.kubernetes.io/managed-by": "Helm",
				"app.kubernetes.io/instance":   "inspect",
				"team":                         "platform",
			},
		},
		"data": map[string]any{"k": "v"},
	}

	values, err := BuildValues(cfg, []map[string]any{doc})
	if err != nil {
		t.Fatalf("BuildValues returned error: %v", err)
	}
	group := values[config.DefaultGroupName].(map[string]any)
	app := group["config-map-demo"].(map[string]any)
	labelsYAML, _ := app["metadata"].(string)
	if labelsYAML == "" {
		t.Fatalf("expected non-empty metadata")
	}
	if containsAny(labelsYAML, "helm.sh/chart", "app.kubernetes.io/managed-by", "app.kubernetes.io/instance") {
		t.Fatalf("expected helm-managed labels to be filtered, got:\n%s", labelsYAML)
	}
	if !containsAny(labelsYAML, "team: platform") {
		t.Fatalf("expected custom label to remain, got:\n%s", labelsYAML)
	}
}

func containsAny(s string, needles ...string) bool {
	for _, n := range needles {
		if strings.Contains(s, n) {
			return true
		}
	}
	return false
}

func TestBuildValuesHelpersExperimental_MapsContainerEnvHelpers(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{"labels": map[string]any{"app": "demo"}},
					"spec": map[string]any{
						"containers": []any{
							map[string]any{
								"name":  "app",
								"image": "nginx:1.25",
								"env": []any{
									map[string]any{"name": "A", "value": "1"},
									map[string]any{"name": "B", "valueFrom": map[string]any{"secretKeyRef": map[string]any{"name": "sec", "key": "B"}}},
								},
								"envFrom": []any{
									map[string]any{"configMapRef": map[string]any{"name": "cm-a"}},
									map[string]any{"secretRef": map[string]any{"name": "sec-a"}},
								},
							},
						},
					},
				},
			},
		},
	}

	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	stateless := values["apps-stateless"].(map[string]any)
	app := stateless["demo"].(map[string]any)
	containers := app["containers"].(map[string]any)
	c := containers["app"].(map[string]any)

	envVars := c["envVars"].(map[string]any)
	if envVars["A"] != "1" {
		t.Fatalf("expected envVars.A=1, got %#v", envVars)
	}
	if _, ok := c["sharedEnvConfigMaps"]; !ok {
		t.Fatalf("expected sharedEnvConfigMaps, got %#v", c)
	}
	if _, ok := c["sharedEnvSecrets"]; !ok {
		t.Fatalf("expected sharedEnvSecrets, got %#v", c)
	}
	envYAML, _ := c["env"].(string)
	if !strings.Contains(envYAML, "secretKeyRef") {
		t.Fatalf("expected complex env to remain in env YAML, got %q", envYAML)
	}
	selectorYAML, _ := app["selector"].(string)
	if strings.Contains(selectorYAML, "matchLabels:") || !strings.Contains(selectorYAML, "app: demo") {
		t.Fatalf("expected deployment selector to be preserved, got %q", selectorYAML)
	}
}

func TestBuildValuesHelpersExperimental_SecretStringDataDoesNotFallback(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "Secret",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"type":       "Opaque",
			"stringData": map[string]any{"A": "1"},
		},
	}

	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-secrets"]; !ok {
		t.Fatalf("expected apps-secrets group, got %#v", values)
	}
	if _, ok := values["imported-manifests"]; ok {
		t.Fatalf("did not expect raw fallback group for simple Secret with stringData: %#v", values)
	}
	sec := values["apps-secrets"].(map[string]any)["demo"].(map[string]any)
	extra, _ := sec["extraFields"].(string)
	if !strings.Contains(extra, "stringData:") || !strings.Contains(extra, "A: \"1\"") && !strings.Contains(extra, "A: '1'") && !strings.Contains(extra, "A: 1") {
		t.Fatalf("expected stringData preserved in extraFields, got: %q", extra)
	}
}

func TestBuildValuesHelpersExperimental_DeploymentImageDigestDoesNotFallback(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{"labels": map[string]any{"app": "demo"}},
					"spec": map[string]any{
						"containers": []any{
							map[string]any{"name": "app", "image": "repo/nginx@sha256:abcdef"},
						},
					},
				},
			},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-stateless"]; !ok {
		t.Fatalf("expected apps-stateless group, got %#v", values)
	}
	if _, ok := values["imported-manifests"]; ok {
		t.Fatalf("did not expect raw fallback for deployment with digest image: %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_IngressWithoutHostDoesNotFallback(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "networking.k8s.io/v1",
			"kind":       "Ingress",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"rules": []any{
					map[string]any{
						"http": map[string]any{
							"paths": []any{
								map[string]any{
									"path":     "/",
									"pathType": "Prefix",
									"backend": map[string]any{"service": map[string]any{
										"name": "svc",
										"port": map[string]any{"number": 80},
									}},
								},
							},
						},
					},
				},
			},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-ingresses"]; !ok {
		t.Fatalf("expected apps-ingresses group, got %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_MapsJobToAppsJobs(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "batch/v1",
			"kind":       "Job",
			"metadata":   map[string]any{"name": "demo-job", "namespace": "default"},
			"spec": map[string]any{
				"template": map[string]any{
					"spec": map[string]any{
						"restartPolicy": "Never",
						"containers": []any{
							map[string]any{"name": "main", "image": "busybox:1.36", "command": []any{"sh", "-c", "echo ok"}},
						},
					},
				},
			},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-jobs"]; !ok {
		t.Fatalf("expected apps-jobs group, got %#v", values)
	}
	if _, ok := values["imported-manifests"]; ok {
		t.Fatalf("did not expect raw fallback for simple Job: %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_MapsServiceAccountRoleRoleBindingToAppsServiceAccounts(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ServiceAccount",
			"metadata":   map[string]any{"name": "demo-sa", "namespace": "demo-ns"},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "Role",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"rules": []any{
				map[string]any{
					"apiGroups": []any{""},
					"resources": []any{"pods"},
					"verbs":     []any{"get", "list"},
				},
			},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "RoleBinding",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"subjects": []any{
				map[string]any{"kind": "ServiceAccount", "name": "demo-sa", "namespace": "demo-ns"},
			},
			"roleRef": map[string]any{
				"apiGroup": "rbac.authorization.k8s.io",
				"kind":     "Role",
				"name":     "demo-role",
			},
		},
	}

	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	saGroup, ok := values["apps-service-accounts"].(map[string]any)
	if !ok || len(saGroup) == 0 {
		t.Fatalf("expected apps-service-accounts group, got %#v", values)
	}
	sa := saGroup["demo-sa"].(map[string]any)
	if sa["name"] != "demo-sa" {
		t.Fatalf("unexpected service account name: %#v", sa["name"])
	}
	if sa["namespace"] != "demo-ns" {
		t.Fatalf("expected custom namespace on SA app: %#v", sa["namespace"])
	}
	roles, ok := sa["roles"].(map[string]any)
	if !ok || len(roles) == 0 {
		t.Fatalf("expected roles attached to service account: %#v", sa)
	}
	role := roles["demo-role"].(map[string]any)
	if role["name"] != "demo-role" {
		t.Fatalf("expected role name override preserved, got %#v", role["name"])
	}
	rules, ok := role["rules"].(map[string]any)
	if !ok || len(rules) == 0 {
		t.Fatalf("expected rules map in attached role: %#v", role)
	}
	if binding, ok := role["binding"].(map[string]any); ok {
		if _, hasSubjects := binding["subjects"]; hasSubjects {
			t.Fatalf("did not expect binding.subjects for default single SA subject: %#v", binding)
		}
	}
	r1 := rules["rule-1"].(map[string]any)
	if _, ok := r1["verbs"]; !ok {
		t.Fatalf("expected verbs in role rule: %#v", r1)
	}
	if _, ok := values[config.DefaultGroupName]; ok {
		t.Fatalf("did not expect generic fallback for simple SA+Role+RoleBinding: %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_ServiceAccountRoleBindingMultiSubjectUsesBindingOverride(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "demo-sa", "namespace": "demo-ns"}},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "Role",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"rules":      []any{map[string]any{"apiGroups": []any{""}, "resources": []any{"pods"}, "verbs": []any{"get"}}},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "RoleBinding",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"subjects": []any{
				map[string]any{"kind": "ServiceAccount", "name": "demo-sa", "namespace": "demo-ns"},
				map[string]any{"kind": "User", "name": "alice"},
			},
			"roleRef": map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "demo-role"},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	saGroup, ok := values["apps-service-accounts"].(map[string]any)
	if !ok {
		t.Fatalf("expected apps-service-accounts, got %#v", values)
	}
	sa := saGroup["demo-sa"].(map[string]any)
	role := sa["roles"].(map[string]any)["demo-role"].(map[string]any)
	binding := role["binding"].(map[string]any)
	if _, ok := binding["subjects"]; !ok {
		t.Fatalf("expected binding.subjects override for multi-subject binding: %#v", binding)
	}
	if subjYAML, _ := binding["subjects"].(string); !strings.Contains(subjYAML, "kind: User") {
		t.Fatalf("expected extra user subject to be preserved in binding.subjects, got %#v", binding)
	}
	if _, ok := values[config.DefaultGroupName]; ok {
		t.Fatalf("did not expect generic fallback for representable multi-subject rolebinding, got %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_ServiceAccountRoleBindingDifferentSATargetsFallback(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "sa1", "namespace": "demo-ns"}},
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "sa2", "namespace": "demo-ns"}},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "Role",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"rules":      []any{map[string]any{"apiGroups": []any{""}, "resources": []any{"pods"}, "verbs": []any{"get"}}},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "RoleBinding",
			"metadata":   map[string]any{"name": "demo-role", "namespace": "demo-ns"},
			"subjects": []any{
				map[string]any{"kind": "ServiceAccount", "name": "sa1", "namespace": "demo-ns"},
				map[string]any{"kind": "ServiceAccount", "name": "sa2", "namespace": "demo-ns"},
			},
			"roleRef": map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "demo-role"},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values[config.DefaultGroupName]; !ok {
		t.Fatalf("expected generic fallback for binding with multiple target SAs, got %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_MapsDefaultNamespaceServiceAccountRBAC(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "apps-k8s-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "demo-sa"}},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "Role",
			"metadata":   map[string]any{"name": "demo-role"},
			"rules": []any{
				map[string]any{"apiGroups": []any{""}, "resources": []any{"configmaps"}, "verbs": []any{"get"}},
			},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "RoleBinding",
			"metadata":   map[string]any{"name": "demo-rb"},
			"subjects": []any{
				map[string]any{"kind": "ServiceAccount", "name": "demo-sa"},
			},
			"roleRef": map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "demo-role"},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-service-accounts"]; !ok {
		t.Fatalf("expected apps-service-accounts group, got %#v", values)
	}
	if _, ok := values["apps-k8s-manifests"]; ok {
		t.Fatalf("did not expect generic fallback for default namespace SA+RBAC: %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_WorkloadBoundServiceAccountUsesAppsServiceAccounts(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "apps-k8s-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{"labels": map[string]any{"app": "demo"}},
					"spec": map[string]any{
						"serviceAccountName": "demo",
						"containers": []any{
							map[string]any{"name": "app", "image": "nginx:1.25"},
						},
					},
				},
			},
		},
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "demo"}},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "Role",
			"metadata":   map[string]any{"name": "demo"},
			"rules": []any{
				map[string]any{"apiGroups": []any{""}, "resources": []any{"pods"}, "verbs": []any{"get"}},
			},
		},
		{
			"apiVersion": "rbac.authorization.k8s.io/v1",
			"kind":       "RoleBinding",
			"metadata":   map[string]any{"name": "demo"},
			"subjects": []any{
				map[string]any{"kind": "ServiceAccount", "name": "demo"},
			},
			"roleRef": map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "demo"},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-service-accounts"]; !ok {
		t.Fatalf("expected apps-service-accounts for workload-bound SA, got %#v", values)
	}
	stateless := values["apps-stateless"].(map[string]any)
	app := stateless["demo"].(map[string]any)
	if _, ok := app["serviceAccount"]; ok {
		t.Fatalf("did not expect inline app.serviceAccount when SA mapped to apps-service-accounts: %#v", app["serviceAccount"])
	}
	if _, ok := values["apps-k8s-manifests"]; ok {
		t.Fatalf("did not expect generic fallback for simple workload-bound SA+RBAC: %#v", values)
	}
}

func TestBuildValuesHelpersExperimental_MapsCronJobToAppsCronJobs(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      "imported-manifests",
		GroupType:      "imported-raw-manifest",
		ImportStrategy: config.ImportStrategyHelpersExperimental,
	}
	docs := []map[string]any{
		{
			"apiVersion": "batch/v1",
			"kind":       "CronJob",
			"metadata":   map[string]any{"name": "demo-cron", "namespace": "default"},
			"spec": map[string]any{
				"schedule": "*/5 * * * *",
				"jobTemplate": map[string]any{
					"spec": map[string]any{
						"template": map[string]any{
							"spec": map[string]any{
								"restartPolicy": "OnFailure",
								"containers": []any{
									map[string]any{"name": "main", "image": "busybox:1.36", "args": []any{"echo", "tick"}},
								},
							},
						},
					},
				},
			},
		},
	}
	values, err := BuildValuesHelpersExperimental(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValuesHelpersExperimental returned error: %v", err)
	}
	if _, ok := values["apps-cronjobs"]; !ok {
		t.Fatalf("expected apps-cronjobs group, got %#v", values)
	}
	if _, ok := values["imported-manifests"]; ok {
		t.Fatalf("did not expect raw fallback for simple CronJob: %#v", values)
	}
}

func TestMapConfigMapToAppsConfigmaps_ServiceAndNetworkPolicyMappers(t *testing.T) {
	t.Run("configmap", func(t *testing.T) {
		key, app, ok := mapConfigMapToAppsConfigmaps(map[string]any{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name":        "cfg",
				"annotations": map[string]any{"a": "1"},
				"labels":      map[string]any{"team": "platform"},
			},
			"data":       map[string]any{"A": "1"},
			"binaryData": map[string]any{"B": "Yg=="},
			"immutable":  true,
			"extraTop":   "x",
		})
		if !ok || key != "cfg" {
			t.Fatalf("expected configmap mapped, key=%q ok=%v app=%#v", key, ok, app)
		}
		if _, ok := app["data"]; !ok {
			t.Fatalf("expected data yaml in app: %#v", app)
		}
		if ef, _ := app["extraFields"].(string); !strings.Contains(ef, "immutable: true") || !strings.Contains(ef, "extraTop: x") {
			t.Fatalf("expected immutable+extraTop in extraFields, got %q", ef)
		}
	})

	t.Run("service", func(t *testing.T) {
		key, app, ok := mapServiceToAppsServices(map[string]any{
			"apiVersion": "v1",
			"kind":       "Service",
			"metadata":   map[string]any{"name": "svc"},
			"spec": map[string]any{
				"type":                "ClusterIP",
				"selector":            map[string]any{"app": "demo"},
				"ports":               []any{map[string]any{"name": "http", "port": 80}},
				"clusterIPs":          []any{"10.0.0.1"},
				"trafficPolicy":       "foo", // unsupported -> extraSpec
				"healthCheckNodePort": 30001,
			},
		})
		if !ok || key != "svc" {
			t.Fatalf("expected service mapped, key=%q ok=%v", key, ok)
		}
		if app["type"] != "ClusterIP" {
			t.Fatalf("expected type copied, got %#v", app["type"])
		}
		if _, ok := app["ports"]; !ok {
			t.Fatalf("expected ports yaml in app: %#v", app)
		}
		if es, _ := app["extraSpec"].(string); !strings.Contains(es, "trafficPolicy: foo") {
			t.Fatalf("expected unsupported field in extraSpec, got %q", es)
		}
	})

	t.Run("networkpolicy", func(t *testing.T) {
		key, app, ok := mapNetworkPolicyToAppsNetworkPolicies(map[string]any{
			"apiVersion": "networking.k8s.io/v1",
			"kind":       "NetworkPolicy",
			"metadata":   map[string]any{"name": "np"},
			"spec": map[string]any{
				"podSelector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"policyTypes": []any{"Ingress"},
				"ingress":     []any{map[string]any{}},
				"egress":      []any{map[string]any{}},
				"custom":      "x",
			},
		})
		if !ok || key != "np" {
			t.Fatalf("expected networkpolicy mapped, key=%q ok=%v", key, ok)
		}
		if app["type"] != "kubernetes" {
			t.Fatalf("expected kubernetes type, got %#v", app["type"])
		}
		if specYAML, _ := app["spec"].(string); !strings.Contains(specYAML, "custom: x") {
			t.Fatalf("expected residual spec in spec field, got %q", specYAML)
		}
	})
}

func TestAttachPDBsAndServiceAccountsToStatelessApps(t *testing.T) {
	stateless := map[string]map[string]any{
		"default/demo": {"name": "demo"},
	}
	rawFallback := []map[string]any{}

	attachPDBsToStatelessApps(stateless, []map[string]any{
		{
			"apiVersion": "policy/v1",
			"kind":       "PodDisruptionBudget",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"minAvailable": 1,
				"selector":     map[string]any{"matchLabels": map[string]any{"app": "demo"}},
			},
		},
		{
			"apiVersion": "policy/v1",
			"kind":       "PodDisruptionBudget",
			"metadata":   map[string]any{"name": "other"},
			"spec":       map[string]any{"maxUnavailable": 1},
		},
	}, &rawFallback)
	app := stateless["default/demo"]
	if _, ok := app["podDisruptionBudget"]; !ok {
		t.Fatalf("expected pdb attached to stateless app: %#v", app)
	}
	if len(rawFallback) != 1 || metadataName(rawFallback[0]) != "other" {
		t.Fatalf("expected unmatched pdb in raw fallback, got %#v", rawFallback)
	}

	rawFallback = nil
	attachServiceAccountsToStatelessApps(stateless, map[string]map[string]any{}, []map[string]any{
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "demo", "namespace": "default"}},
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "noapp", "namespace": "default"}},
	}, &rawFallback)
	if _, ok := stateless["default/demo"]["serviceAccount"]; !ok {
		t.Fatalf("expected serviceAccount attached inline")
	}
	if len(rawFallback) != 1 || metadataName(rawFallback[0]) != "noapp" {
		t.Fatalf("expected unmatched serviceaccount in fallback, got %#v", rawFallback)
	}

	// If mapped in apps-service-accounts already, inline attachment should be skipped.
	delete(stateless["default/demo"], "serviceAccount")
	rawFallback = nil
	attachServiceAccountsToStatelessApps(stateless, map[string]map[string]any{"default/demo": {"name": "demo"}}, []map[string]any{
		{"apiVersion": "v1", "kind": "ServiceAccount", "metadata": map[string]any{"name": "demo", "namespace": "default"}},
	}, &rawFallback)
	if _, ok := stateless["default/demo"]["serviceAccount"]; ok {
		t.Fatalf("did not expect inline serviceAccount when apps-service-accounts entry exists")
	}
}

func TestAttachClusterRoleDocToServiceAccount_AndAttachRBACClusterRoleFlow(t *testing.T) {
	saApp := map[string]any{"name": "sa"}
	crDoc := map[string]any{
		"apiVersion": "rbac.authorization.k8s.io/v1",
		"kind":       "ClusterRole",
		"metadata":   map[string]any{"name": "demo-cr"},
		"rules":      []any{map[string]any{"apiGroups": []any{""}, "resources": []any{"pods"}, "verbs": []any{"get"}}},
	}
	crbDoc := map[string]any{
		"apiVersion": "rbac.authorization.k8s.io/v1",
		"kind":       "ClusterRoleBinding",
		"metadata":   map[string]any{"name": "demo-cr"},
		"subjects":   []any{map[string]any{"kind": "ServiceAccount", "name": "sa"}},
		"roleRef":    map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": "demo-cr"},
	}
	if !attachClusterRoleDocToServiceAccount(saApp, crDoc, crbDoc, "demo-cr", "ClusterRole") {
		t.Fatalf("expected attachClusterRoleDocToServiceAccount to succeed")
	}
	crs, _ := saApp["clusterRoles"].(map[string]any)
	if len(crs) != 1 {
		t.Fatalf("expected clusterRoles attached, got %#v", saApp)
	}
	if !rbAttached(crDoc) {
		t.Fatalf("expected clusterrole marked attached")
	}

	// end-to-end attachRBACToServiceAccounts cluster role path
	saApps := map[string]map[string]any{"default/sa2": {"name": "sa2"}}
	crDoc2 := map[string]any{
		"apiVersion": "rbac.authorization.k8s.io/v1",
		"kind":       "ClusterRole",
		"metadata":   map[string]any{"name": "global-reader"},
		"rules":      []any{map[string]any{"apiGroups": []any{""}, "resources": []any{"namespaces"}, "verbs": []any{"get"}}},
	}
	crbDoc2 := map[string]any{
		"apiVersion": "rbac.authorization.k8s.io/v1",
		"kind":       "ClusterRoleBinding",
		"metadata":   map[string]any{"name": "global-reader"},
		"subjects":   []any{map[string]any{"kind": "ServiceAccount", "name": "sa2"}},
		"roleRef":    map[string]any{"apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": "global-reader"},
	}
	var raw []map[string]any
	attachRBACToServiceAccounts(saApps, nil, nil, []map[string]any{crDoc2}, []map[string]any{crbDoc2}, &raw)
	if len(raw) != 0 {
		t.Fatalf("expected no fallback for attachable clusterrole rbac, got %#v", raw)
	}
	if _, ok := saApps["default/sa2"]["clusterRoles"]; !ok {
		t.Fatalf("expected clusterRoles attached via attachRBACToServiceAccounts")
	}
}

func TestConvertHelpers_IncludeBodyCanonicalAndMergeHelpers(t *testing.T) {
	if _, err := includeBody(includeDescriptor{Kind: "unknown"}); err == nil {
		t.Fatalf("expected error for unknown include kind")
	}
	body, err := includeBody(includeDescriptor{Kind: "app_scalars_hash", Canonical: `{"immutable":true,"type":"Opaque"}`})
	if err != nil {
		t.Fatalf("includeBody scalars hash error: %v", err)
	}
	if body["type"] != "Opaque" || body["immutable"] != true {
		t.Fatalf("unexpected include body: %#v", body)
	}
	if _, err := canonicalJSON(map[string]any{"b": 2, "a": 1}); err != nil {
		t.Fatalf("canonicalJSON error: %v", err)
	}
	if got := includeName(includeDescriptor{Kind: "app_scalars_hash"}, 2); got != "imported-scalars-2" {
		t.Fatalf("unexpected includeName: %q", got)
	}

	group := map[string]any{"x": 1, "x-2": 1}
	if got := dedupeGroupKey(group, "x"); got != "x-3" {
		t.Fatalf("unexpected dedupeGroupKey: %q", got)
	}

	dst := map[string]any{
		"global": map[string]any{
			"env":       "dev",
			"_includes": map[string]any{"a": map[string]any{"k": "v"}},
		},
		"apps-k8s-manifests": map[string]any{"one": 1},
	}
	src := map[string]any{
		"global": map[string]any{
			"env":       "prod",
			"_includes": map[string]any{"b": map[string]any{"k": "v2"}},
		},
		"apps-stateless": map[string]any{"demo": map[string]any{"enabled": true}},
	}
	mergeTopLevelValues(dst, src)
	global := dst["global"].(map[string]any)
	if global["env"] != "prod" {
		t.Fatalf("expected global.env override, got %#v", global["env"])
	}
	includes := global["_includes"].(map[string]any)
	if _, ok := includes["a"]; !ok {
		t.Fatalf("expected existing include kept")
	}
	if _, ok := includes["b"]; !ok {
		t.Fatalf("expected new include merged")
	}
	if _, ok := dst["apps-stateless"]; !ok {
		t.Fatalf("expected non-global section merged")
	}
}

func TestExtractServiceExtraSpec_AndIncludeBodyAppField(t *testing.T) {
	spec := map[string]any{
		"ports":    []any{map[string]any{"port": 80}},
		"selector": map[string]any{"app": "x"},
		"foo":      "bar",
	}
	extra := extractServiceExtraSpec(spec)
	if _, ok := extra["foo"]; !ok {
		t.Fatalf("expected unsupported key in extra service spec, got %#v", extra)
	}
	if _, ok := extra["ports"]; ok {
		t.Fatalf("did not expect handled field in extra service spec, got %#v", extra)
	}

	body, err := includeBody(includeDescriptor{Kind: "app_field", Field: "metadata", Canonical: "name: x"})
	if err != nil {
		t.Fatalf("includeBody app_field error: %v", err)
	}
	if body["metadata"] != "name: x" {
		t.Fatalf("unexpected include body for app_field: %#v", body)
	}
}
