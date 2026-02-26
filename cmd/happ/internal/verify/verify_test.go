package verify

import (
	"strings"
	"testing"
)

func TestEquivalent_IgnoresMetadataLabelsAnnotationsAndStatus(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name":        "demo",
				"namespace":   "default",
				"labels":      map[string]any{"a": "1"},
				"annotations": map[string]any{"x": "y"},
			},
			"data":   map[string]any{"k": "v"},
			"status": map[string]any{"ignored": true},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name":        "demo",
				"namespace":   "default",
				"labels":      map[string]any{"a": "2", "helm-apps/rewrite": "1"},
				"annotations": map[string]any{"another": "value"},
			},
			"data": map[string]any{"k": "v"},
		},
	}

	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected equivalent, got: %+v", res)
	}
}

func TestEquivalent_IgnoresInternalDoubleUnderscoreFields(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion":             "v1",
			"kind":                   "ConfigMap",
			"metadata":               map[string]any{"name": "demo", "namespace": "default"},
			"data":                   map[string]any{"k": "v"},
			"__importerRBACAttached": true,
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "demo"},
			"data":       map[string]any{"k": "v"},
		},
	}
	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected internal __ fields to be ignored, got: %+v", res)
	}
}

func TestEquivalent_DetectsMismatch(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"data":       map[string]any{"k": "v1"},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"data":       map[string]any{"k": "v2"},
		},
	}

	res := Equivalent(src, gen)
	if res.Equal {
		t.Fatalf("expected mismatch, got: %+v", res)
	}
	if res.Summary == "" {
		t.Fatalf("expected mismatch summary")
	}
	if !strings.Contains(res.Summary, "data.k") {
		t.Fatalf("expected field path in summary, got: %s", res.Summary)
	}
}

func TestEquivalent_TreatsEmptyNamespaceAsDefault(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "Service",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec":       map[string]any{"type": "ClusterIP"},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "Service",
			"metadata":   map[string]any{"name": "demo"},
			"spec":       map[string]any{"type": "ClusterIP"},
		},
	}

	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected equivalent for default/empty namespace, got: %+v", res)
	}
}

func TestEquivalent_TreatsNullFieldAsAbsent(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"data":       nil,
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "demo"},
		},
	}

	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected null field and absent field to be equivalent, got: %+v", res)
	}
}

func TestEquivalent_IgnoresPodTemplateMetadataName(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{},
					"spec":     map[string]any{"containers": []any{map[string]any{"name": "app", "image": "nginx"}}},
				},
			},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{"name": "demo"},
					"spec":     map[string]any{"containers": []any{map[string]any{"name": "app", "image": "nginx"}}},
				},
			},
		},
	}

	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected pod template metadata.name to be ignored, got: %+v", res)
	}
}

func TestEquivalent_IgnoresEnvOrderInContainerList(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo", "namespace": "default"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{},
					"spec": map[string]any{
						"containers": []any{
							map[string]any{
								"name":  "app",
								"image": "nginx",
								"env": []any{
									map[string]any{"name": "POD_NAME", "valueFrom": map[string]any{"fieldRef": map[string]any{"fieldPath": "metadata.name"}}},
									map[string]any{"name": "LD_PRELOAD", "value": "x"},
								},
							},
						},
					},
				},
			},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "apps/v1",
			"kind":       "Deployment",
			"metadata":   map[string]any{"name": "demo"},
			"spec": map[string]any{
				"selector": map[string]any{"matchLabels": map[string]any{"app": "demo"}},
				"template": map[string]any{
					"metadata": map[string]any{"name": "demo"},
					"spec": map[string]any{
						"containers": []any{
							map[string]any{
								"name":  "app",
								"image": "nginx",
								"env": []any{
									map[string]any{"name": "LD_PRELOAD", "value": "x"},
									map[string]any{"name": "POD_NAME", "valueFrom": map[string]any{"fieldRef": map[string]any{"fieldPath": "metadata.name"}}},
								},
							},
						},
					},
				},
			},
		},
	}

	res := Equivalent(src, gen)
	if !res.Equal {
		t.Fatalf("expected env order difference to be ignored, got: %+v", res)
	}
}

func TestCompareDetailed_AndNormalizeDocsForCompare(t *testing.T) {
	src := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name":      "same",
				"namespace": "default",
				"labels":    map[string]any{"x": "1"},
			},
			"data": map[string]any{"k": "v"},
		},
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "changed", "namespace": "default"},
			"data":       map[string]any{"k": "v1"},
		},
		{
			"apiVersion": "v1",
			"kind":       "Secret",
			"metadata":   map[string]any{"name": "missing", "namespace": "default"},
			"data":       map[string]any{"x": "eA=="},
		},
	}
	gen := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "same"},
			"data":       map[string]any{"k": "v"},
		},
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata":   map[string]any{"name": "changed"},
			"data":       map[string]any{"k": "v2"},
		},
		{
			"apiVersion": "v1",
			"kind":       "Service",
			"metadata":   map[string]any{"name": "extra"},
			"spec":       map[string]any{"type": "ClusterIP"},
		},
	}

	norm := NormalizeDocsForCompare(src)
	if len(norm) != 3 {
		t.Fatalf("NormalizeDocsForCompare len mismatch: %d", len(norm))
	}
	if meta, _ := norm[0]["metadata"].(map[string]any); meta != nil {
		if _, ok := meta["labels"]; ok {
			t.Fatalf("expected labels removed in normalized docs: %#v", meta)
		}
	}

	res := CompareDetailed(src, gen)
	if res.Equal {
		t.Fatalf("expected non-equal detailed compare")
	}
	if !strings.Contains(res.Summary, "mismatches=") {
		t.Fatalf("unexpected summary: %q", res.Summary)
	}
	statuses := map[string]int{}
	for _, r := range res.Resources {
		statuses[r.Status]++
	}
	if statuses["equal"] == 0 || statuses["changed"] == 0 || statuses["missing_in_generated"] == 0 || statuses["extra_in_generated"] == 0 {
		t.Fatalf("expected all statuses represented, got %#v", statuses)
	}
}

func TestSemanticListItemKey_AndHelpers(t *testing.T) {
	if k, ok := semanticListItemKey(map[string]any{"name": "app"}); !ok || k != "name:app" {
		t.Fatalf("unexpected key for named item: %q ok=%v", k, ok)
	}
	if k, ok := semanticListItemKey(map[string]any{"configMapRef": map[string]any{"name": "cm1"}}); !ok || k != "configMapRef:cm1" {
		t.Fatalf("unexpected key for envFrom configMapRef: %q ok=%v", k, ok)
	}
	if k, ok := semanticListItemKey(map[string]any{"secretRef": map[string]any{"name": "sec1"}}); !ok || k != "secretRef:sec1" {
		t.Fatalf("unexpected key for envFrom secretRef: %q ok=%v", k, ok)
	}
	if k, ok := semanticListItemKey(map[string]any{"containerPort": 8080, "protocol": "TCP"}); !ok || !strings.Contains(k, "containerPort:8080") {
		t.Fatalf("unexpected key for port item: %q ok=%v", k, ok)
	}
	if _, ok := semanticListItemKey("bad"); ok {
		t.Fatalf("expected non-map item to be unsupported")
	}

	if v, ok := nestedStrField(map[string]any{"a": map[string]any{"b": "x"}}, "a", "b"); !ok || v != "x" {
		t.Fatalf("unexpected nestedStrField result: %q ok=%v", v, ok)
	}
	if v, ok := intLikeField(map[string]any{"p": 8080}, "p"); !ok || v != "8080" {
		t.Fatalf("unexpected intLikeField int result: %q ok=%v", v, ok)
	}
	if v, ok := intLikeField(map[string]any{"p": 8080.0}, "p"); !ok || v != "8080" {
		t.Fatalf("unexpected intLikeField float result: %q ok=%v", v, ok)
	}
	if v, ok := intLikeField(map[string]any{"p": "http"}, "p"); !ok || v != "http" {
		t.Fatalf("unexpected intLikeField string result: %q ok=%v", v, ok)
	}
}

func TestSortSemanticList_DuplicateAndUnsupported(t *testing.T) {
	if _, ok := sortSemanticList([]any{
		map[string]any{"name": "a"},
		map[string]any{"name": "a"},
	}); ok {
		t.Fatalf("expected duplicate semantic keys to disable sorting")
	}
	if _, ok := sortSemanticList([]any{
		map[string]any{"name": "a"},
		"bad",
	}); ok {
		t.Fatalf("expected unsupported item to disable sorting")
	}
	out, ok := sortSemanticList([]any{
		map[string]any{"name": "b"},
		map[string]any{"name": "a"},
	})
	if !ok {
		t.Fatalf("expected sortable list")
	}
	first := out[0].(map[string]any)["name"]
	if first != "a" {
		t.Fatalf("expected sorted order by semantic key, got %#v", out)
	}
}

func TestSortRecAndSortedMapKeysAny(t *testing.T) {
	in := map[string]any{"b": map[string]any{"d": 2, "c": 1}, "a": []any{map[string]any{"z": 1, "y": 2}}}
	out := sortRec(in).(map[string]any)
	keys := sortedMapKeysAny(out)
	if len(keys) != 2 || keys[0] != "a" || keys[1] != "b" {
		t.Fatalf("unexpected sortedMapKeysAny: %#v", keys)
	}
	if _, err := canonicalJSON(out); err != nil {
		t.Fatalf("canonicalJSON error: %v", err)
	}
}

func TestDyffWrappers_InternalDiffLike(t *testing.T) {
	if !DyffAvailable() {
		t.Fatalf("expected embedded dyff-like to be available")
	}
	diff, err := DyffBetweenYAML([]byte("a: 1\n"), []byte("a: 2\n"))
	if err != nil {
		t.Fatalf("DyffBetweenYAML error: %v", err)
	}
	if !strings.Contains(diff, "~ $.a: 1 -> 2") {
		t.Fatalf("unexpected dyff-like output: %q", diff)
	}
	empty, err := DyffBetweenYAML([]byte("a: 1\n"), []byte("a: 1\n"))
	if err != nil {
		t.Fatalf("DyffBetweenYAML equal error: %v", err)
	}
	if strings.TrimSpace(empty) != "" {
		t.Fatalf("expected empty diff for equal files, got %q", empty)
	}
}

func TestDyffBetweenDocs_Embedded(t *testing.T) {
	diff, err := DyffBetweenDocs(
		map[string]any{"apiVersion": "v1", "kind": "ConfigMap", "metadata": map[string]any{"name": "a"}, "data": map[string]any{"k": "1"}},
		map[string]any{"apiVersion": "v1", "kind": "ConfigMap", "metadata": map[string]any{"name": "a"}, "data": map[string]any{"k": "2"}},
	)
	if err != nil {
		t.Fatalf("DyffBetweenDocs error: %v", err)
	}
	if !strings.Contains(diff, "$.data.k") {
		t.Fatalf("unexpected diff output: %q", diff)
	}
}
