package inspectweb

import (
	"bytes"
	"errors"
	"io"
	"net"
	"net/http"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/composeinspect"
	"github.com/zol/helm-apps/cmd/happ/internal/verify"
)

func TestBuildModel_DetectsBasicRelations(t *testing.T) {
	docs := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "app", "namespace": "default"},
			"spec": map[string]any{
				"template": map[string]any{
					"metadata": map[string]any{"labels": map[string]any{"app": "demo"}},
					"spec": map[string]any{
						"volumes": []any{
							map[string]any{"configMap": map[string]any{"name": "cfg"}},
						},
					},
				},
			},
		},
		{
			"apiVersion": "v1",
			"kind":       "Service",
			"metadata":   map[string]any{"name": "svc", "namespace": "default"},
			"spec":       map[string]any{"selector": map[string]any{"app": "demo"}},
		},
		{
			"apiVersion": "networking.k8s.io/v1",
			"kind":       "Ingress",
			"metadata":   map[string]any{"name": "ing", "namespace": "default"},
			"spec": map[string]any{
				"rules": []any{
					map[string]any{"http": map[string]any{"paths": []any{
						map[string]any{"backend": map[string]any{"service": map[string]any{"name": "svc"}}},
					}}},
				},
			},
		},
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "cfg", "namespace": "default"},
			"data":       map[string]any{"A": "1"},
		},
	}

	m := BuildModel(docs, nil, nil)
	if m.Summary.ResourceCount != 4 {
		t.Fatalf("unexpected resource count: %+v", m.Summary)
	}
	assertHasRel(t, m, "routes-to-service")
	assertHasRel(t, m, "selects-workload")
	assertHasRel(t, m, "mounts-configmap")
}

func TestBuildModel_SummaryYAMLOmitsNullSpecAndData(t *testing.T) {
	docs := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ServiceAccount",
			"metadata": map[string]any{
				"name":      "sa",
				"namespace": "default",
				"labels":    map[string]any{"app": "demo"},
			},
		},
	}
	m := BuildModel(docs, nil, nil)
	if len(m.Resources) != 1 {
		t.Fatalf("unexpected resources: %#v", m.Resources)
	}
	s := m.Resources[0].SummaryYAML
	if strings.Contains(s, "spec: null") || strings.Contains(s, "data: null") {
		t.Fatalf("summary YAML must omit null spec/data, got:\n%s", s)
	}
	if !strings.Contains(s, "labels:") {
		t.Fatalf("expected labels in summary YAML, got:\n%s", s)
	}
}

func assertHasRel(t *testing.T, m Model, typ string) {
	t.Helper()
	for _, r := range m.Relations {
		if r.Type == typ {
			return
		}
	}
	t.Fatalf("missing relation type %q in %#v", typ, m.Relations)
}

func TestDiffModelsAndHelpers(t *testing.T) {
	base := Model{Resources: []Resource{
		{ID: "a", Name: "cfg", Namespace: "default", Kind: "ConfigMap", Raw: map[string]any{"k": "1"}},
		{ID: "b", Name: "svc", Kind: "Service", Raw: map[string]any{"k": "1"}},
	}}
	next := Model{Resources: []Resource{
		{ID: "a", Name: "cfg", Namespace: "default", Kind: "ConfigMap", Raw: map[string]any{"k": "2"}},
		{ID: "c", Name: "ing", Namespace: "ns1", Kind: "Ingress", Raw: map[string]any{"k": "1"}},
	}}
	diff := DiffModels(base, next)
	if len(diff.Added) != 1 || len(diff.Changed) != 1 || len(diff.Removed) != 1 {
		t.Fatalf("unexpected diff: %+v", diff)
	}
	if !resourceEqual(Resource{Raw: map[string]any{"a": 1}}, Resource{Raw: map[string]any{"a": 1}}) {
		t.Fatalf("expected resources equal")
	}
	if resourceEqual(Resource{Raw: map[string]any{"a": 1}}, Resource{Raw: map[string]any{"a": 2}}) {
		t.Fatalf("expected resources not equal")
	}
	if bytesEqual([]byte("ab"), []byte("ac")) {
		t.Fatalf("expected bytes not equal")
	}
	if got := shortResourceLabel(Resource{Name: "svc", Kind: "Service"}); got != "svc (Service)" {
		t.Fatalf("unexpected shortResourceLabel: %q", got)
	}
	if got := shortResourceLabel(Resource{Name: "cfg", Namespace: "default", Kind: "ConfigMap"}); got != "default/cfg (ConfigMap)" {
		t.Fatalf("unexpected namespaced shortResourceLabel: %q", got)
	}
}

func TestImportTargetAndWorkloadHintsHelpers(t *testing.T) {
	tests := []struct {
		kind   string
		target string
		mode   string
	}{
		{"Deployment", "apps-stateless.<app>", "helper-candidate"},
		{"StatefulSet", "apps-stateful.<app>", "helper-candidate"},
		{"Job", "apps-jobs.<app>", "helper-candidate"},
		{"CronJob", "apps-cronjobs.<app>", "helper-candidate"},
		{"Service", "apps-services.<name>", "helper-candidate"},
		{"Ingress", "apps-ingresses.<name>", "helper-candidate"},
		{"ConfigMap", "apps-configmaps.<name>", "helper-candidate"},
		{"Secret", "apps-secrets.<name>", "helper-candidate"},
		{"IngressClass", "imported-manifests.<name>", "raw-fallback"},
	}
	for _, tt := range tests {
		target, mode, _ := importTargetHint(Resource{Kind: tt.kind})
		if target != tt.target || mode != tt.mode {
			t.Fatalf("%s -> target=%q mode=%q (want %q %q)", tt.kind, target, mode, tt.target, tt.mode)
		}
	}
	if workloadTypeHint("Deployment") != "stateless" || workloadTypeHint("StatefulSet") != "stateful" || workloadTypeHint("Job") != "job" || workloadTypeHint("DaemonSet") != "stateless" {
		t.Fatalf("unexpected workloadTypeHint")
	}
}

func TestWorkloadHelpersAndYamlString(t *testing.T) {
	doc := map[string]any{
		"apiVersion": "apps/v1",
		"kind":       "Deployment",
		"metadata":   map[string]any{"name": "demo"},
		"spec": map[string]any{
			"template": map[string]any{
				"metadata": map[string]any{"labels": map[string]any{"app": "demo"}},
				"spec": map[string]any{
					"serviceAccountName": "sa1",
					"containers":         []any{map[string]any{"name": "c1"}},
					"initContainers":     []any{map[string]any{"name": "i1"}},
				},
			},
		},
	}
	if !labelsMatch(map[string]string{"app": "demo"}, workloadPodLabels(doc)) {
		t.Fatalf("expected labelsMatch true")
	}
	if labelsMatch(map[string]string{}, map[string]string{"a": "b"}) {
		t.Fatalf("expected labelsMatch false for empty selector")
	}
	if workloadServiceAccountName(doc) != "sa1" {
		t.Fatalf("unexpected SA name")
	}
	refs := workloadRefs(map[string]any{
		"spec": map[string]any{
			"template": map[string]any{
				"spec": map[string]any{
					"volumes": []any{
						map[string]any{"configMap": map[string]any{"name": "cfg"}},
						map[string]any{"secret": map[string]any{"secretName": "sec"}},
					},
					"containers": []any{
						map[string]any{"envFrom": []any{
							map[string]any{"configMapRef": map[string]any{"name": "cfg2"}},
							map[string]any{"secretRef": map[string]any{"name": "sec2"}},
						}},
					},
				},
			},
		},
	})
	if len(refs) < 4 {
		t.Fatalf("expected refs extracted, got %#v", refs)
	}
	if got := yamlString(map[string]any{}); !strings.Contains(got, "no spec/data/labels summary") {
		t.Fatalf("unexpected yamlString for empty map: %q", got)
	}
	if got := yamlString(map[string]any{"a": 1}); !strings.Contains(got, "a: 1") {
		t.Fatalf("unexpected yamlString output: %q", got)
	}
}

func TestIngressServiceNamesAndSelectorsHelpers(t *testing.T) {
	ing := map[string]any{
		"spec": map[string]any{
			"defaultBackend": map[string]any{"service": map[string]any{"name": "svc-default"}},
			"rules": []any{
				map[string]any{"http": map[string]any{"paths": []any{
					map[string]any{"backend": map[string]any{"service": map[string]any{"name": "svc-a"}}},
					map[string]any{"backend": map[string]any{"service": map[string]any{"name": "svc-a"}}},
				}}},
			},
		},
	}
	names := ingressServiceNames(ing)
	if len(names) != 2 || !containsString(names, "svc-default") || !containsString(names, "svc-a") {
		t.Fatalf("unexpected ingressServiceNames: %#v", names)
	}
	sel := serviceSelector(map[string]any{"spec": map[string]any{"selector": map[string]any{"app": "demo", "n": 1}}})
	if len(sel) != 1 || sel["app"] != "demo" {
		t.Fatalf("unexpected serviceSelector: %#v", sel)
	}
	if !isWorkloadKind("CronJob") || isWorkloadKind("ConfigMap") {
		t.Fatalf("unexpected isWorkloadKind behavior")
	}
	if nsNameKey("ns", "n") != "ns/n" {
		t.Fatalf("unexpected nsNameKey")
	}
	if firstNonNil(nil, "x", "y") != "x" {
		t.Fatalf("unexpected firstNonNil")
	}
	if len(mapAny(nil)) != 0 {
		t.Fatalf("expected empty map from mapAny(nil)")
	}
}

func TestServe_HTTPAPIAndHooks(t *testing.T) {
	addr := mustFreeAddr(t)
	model := Model{
		Resources: []Resource{{ID: "v1/ConfigMap/default/demo", Kind: "ConfigMap", Name: "demo", Namespace: "default", Raw: map[string]any{"apiVersion": "v1", "kind": "ConfigMap", "metadata": map[string]any{"name": "demo"}}}},
		Summary:   Summary{ResourceCount: 1},
	}
	var mu sync.Mutex
	var savedPath string
	var savedValues string
	hooks := &InteractiveHooks{
		Enabled:           true,
		SourceValuesYAML:  "a: 1\n",
		SuggestedSavePath: "/tmp/values.inspect.yaml",
		RunExperiment: func(valuesYAML string) (ExperimentResponse, error) {
			if valuesYAML == "bad" {
				return ExperimentResponse{}, errors.New("bad values")
			}
			return ExperimentResponse{Model: model, Diff: ExperimentDiff{Changed: []string{"demo (ConfigMap)"}}}, nil
		},
		CompareEntities: func(valuesYAML string) (CompareEntitiesResponse, error) {
			if valuesYAML == "bad" {
				return CompareEntitiesResponse{}, errors.New("bad compare")
			}
			return CompareEntitiesResponse{
				Compare: verify.DetailedResult{
					Equal:   false,
					Summary: "mismatch",
					Resources: []verify.ResourceComparison{
						{Key: "v1/ConfigMap/default/demo", Status: "changed", DiffPath: "data.k"},
					},
				},
				SourceYAMLByKey:    map[string]string{"v1/ConfigMap/default/demo": "a: 1"},
				GeneratedYAMLByKey: map[string]string{"v1/ConfigMap/default/demo": "a: 2"},
			}, nil
		},
		SaveValues: func(path, valuesYAML string) error {
			if path == "" {
				return errors.New("path required")
			}
			mu.Lock()
			savedPath, savedValues = path, valuesYAML
			mu.Unlock()
			return nil
		},
	}

	done := make(chan error, 1)
	go func() { done <- Serve(addr, false, model, hooks) }()
	waitHTTPReady(t, "http://"+addr+"/api/model")

	getMustStatus(t, "GET", "http://"+addr+"/", nil, http.StatusOK)
	getMustStatus(t, "GET", "http://"+addr+"/preview", nil, http.StatusOK)

	body := getMustStatus(t, "GET", "http://"+addr+"/api/model", nil, http.StatusOK)
	if !bytes.Contains(body, []byte(`"resourceCount":1`)) {
		t.Fatalf("unexpected /api/model body: %s", string(body))
	}
	body = getMustStatus(t, "GET", "http://"+addr+"/api/model.yaml", nil, http.StatusOK)
	if !bytes.Contains(body, []byte("resourcecount: 1")) {
		t.Fatalf("unexpected /api/model.yaml body: %s", string(body))
	}
	body = getMustStatus(t, "GET", "http://"+addr+"/api/source-values", nil, http.StatusOK)
	if !bytes.Contains(body, []byte(`"interactiveEnabled":true`)) {
		t.Fatalf("unexpected /api/source-values body: %s", string(body))
	}
	getMustStatus(t, "GET", "http://"+addr+"/api/experiment/render", nil, http.StatusMethodNotAllowed)
	getMustStatus(t, "GET", "http://"+addr+"/api/experiment/save", nil, http.StatusMethodNotAllowed)
	getMustStatus(t, "GET", "http://"+addr+"/api/experiment/compare", nil, http.StatusMethodNotAllowed)

	body = getMustStatus(t, "POST", "http://"+addr+"/api/experiment/render", strings.NewReader(`{"valuesYAML":"ok"}`), http.StatusOK)
	if !bytes.Contains(body, []byte(`"changed":["demo (ConfigMap)"]`)) {
		t.Fatalf("unexpected experiment render response: %s", string(body))
	}
	body = getMustStatus(t, "POST", "http://"+addr+"/api/experiment/render", strings.NewReader(`{"valuesYAML":"bad"}`), http.StatusBadRequest)
	if !bytes.Contains(body, []byte(`"error":"bad values"`)) {
		t.Fatalf("unexpected experiment render error response: %s", string(body))
	}

	body = getMustStatus(t, "POST", "http://"+addr+"/api/experiment/compare", strings.NewReader(`{"valuesYAML":"ok"}`), http.StatusOK)
	if !bytes.Contains(body, []byte(`"diffPath":"data.k"`)) {
		t.Fatalf("unexpected compare response: %s", string(body))
	}
	body = getMustStatus(t, "POST", "http://"+addr+"/api/experiment/compare", strings.NewReader(`{"valuesYAML":"bad"}`), http.StatusBadRequest)
	if !bytes.Contains(body, []byte(`"error":"bad compare"`)) {
		t.Fatalf("unexpected compare error response: %s", string(body))
	}

	body = getMustStatus(t, "POST", "http://"+addr+"/api/experiment/save", strings.NewReader(`{"path":"/tmp/x.yaml","valuesYAML":"z: 1\n"}`), http.StatusOK)
	if !bytes.Contains(body, []byte(`"ok":true`)) {
		t.Fatalf("unexpected save response: %s", string(body))
	}
	mu.Lock()
	if savedPath != "/tmp/x.yaml" || savedValues != "z: 1\n" {
		t.Fatalf("save hook not called as expected: path=%q values=%q", savedPath, savedValues)
	}
	mu.Unlock()
	getMustStatus(t, "POST", "http://"+addr+"/api/experiment/save", strings.NewReader(`{"path":"","valuesYAML":"x"}`), http.StatusBadRequest)

	getMustStatus(t, "GET", "http://"+addr+"/api/shutdown", nil, http.StatusMethodNotAllowed)
	getMustStatus(t, "POST", "http://"+addr+"/api/shutdown", nil, http.StatusOK)

	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("Serve returned error: %v", err)
		}
	case <-time.After(3 * time.Second):
		t.Fatalf("timeout waiting for Serve shutdown")
	}
}

func TestServeCompose_HTTPAPI(t *testing.T) {
	addr := mustFreeAddr(t)
	rep := composeinspect.Report{
		SourcePath: "docker-compose.yml",
		Services: []composeinspect.ServiceNode{
			{Name: "web", Image: "nginx:1.25"},
		},
		Summary: composeinspect.Summary{ServiceCount: 1},
	}

	done := make(chan error, 1)
	go func() {
		done <- ServeCompose(addr, false, rep, "services:\n  web:\n    image: nginx\n", "global:\n  env: dev\n", "")
	}()
	waitHTTPReady(t, "http://"+addr+"/api/compose-report")

	getMustStatus(t, "GET", "http://"+addr+"/", nil, http.StatusOK)
	body := getMustStatus(t, "GET", "http://"+addr+"/api/compose-report", nil, http.StatusOK)
	if !bytes.Contains(body, []byte(`"sourcePath":"docker-compose.yml"`)) {
		t.Fatalf("unexpected compose-report json: %s", string(body))
	}
	body = getMustStatus(t, "GET", "http://"+addr+"/api/compose-report.yaml", nil, http.StatusOK)
	if !bytes.Contains(body, []byte("sourcePath: docker-compose.yml")) {
		t.Fatalf("unexpected compose-report yaml: %s", string(body))
	}
	getMustStatus(t, "GET", "http://"+addr+"/api/shutdown", nil, http.StatusMethodNotAllowed)
	getMustStatus(t, "POST", "http://"+addr+"/api/shutdown", nil, http.StatusOK)
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("ServeCompose returned error: %v", err)
		}
	case <-time.After(3 * time.Second):
		t.Fatalf("timeout waiting for ServeCompose shutdown")
	}
}

func mustFreeAddr(t *testing.T) string {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen for free addr: %v", err)
	}
	addr := ln.Addr().String()
	_ = ln.Close()
	return addr
}

func waitHTTPReady(t *testing.T, url string) {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		resp, err := http.Get(url)
		if err == nil {
			_, _ = io.Copy(io.Discard, resp.Body)
			_ = resp.Body.Close()
			return
		}
		time.Sleep(30 * time.Millisecond)
	}
	t.Fatalf("server did not become ready: %s", url)
}

func getMustStatus(t *testing.T, method, url string, body io.Reader, wantStatus int) []byte {
	t.Helper()
	req, err := http.NewRequest(method, url, body)
	if err != nil {
		t.Fatalf("new request: %v", err)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("http request %s %s failed: %v", method, url, err)
	}
	defer resp.Body.Close()
	b, _ := io.ReadAll(resp.Body)
	if resp.StatusCode != wantStatus {
		t.Fatalf("%s %s: status=%d want=%d body=%s", method, url, resp.StatusCode, wantStatus, string(b))
	}
	return b
}
