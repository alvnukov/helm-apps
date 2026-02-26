package convert

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
	"gopkg.in/yaml.v3"
)

type literalString string

func (s literalString) MarshalYAML() (any, error) {
	return &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: string(s), Style: yaml.LiteralStyle}, nil
}

func TestBuildValues_Golden(t *testing.T) {
	cfg := config.Config{
		Env:             "prod",
		GroupName:       "imported-manifests",
		GroupType:       "imported-raw-manifest",
		MinIncludeBytes: 1,
	}
	docs := []map[string]any{
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name": "app-a",
				"labels": map[string]any{
					"team": "platform",
					"app":  "demo",
				},
			},
			"data": map[string]any{
				"MODE": "prod",
				"A":    "1",
			},
		},
		{
			"apiVersion": "v1",
			"kind":       "ConfigMap",
			"metadata": map[string]any{
				"name": "app-b",
				"labels": map[string]any{
					"team": "platform",
					"app":  "demo",
				},
			},
			"data": map[string]any{
				"MODE": "prod",
				"A":    "1",
			},
		},
	}

	values, err := BuildValues(cfg, docs)
	if err != nil {
		t.Fatalf("BuildValues returned error: %v", err)
	}

	got, err := yaml.Marshal(deepLiteralizeForTest(values))
	if err != nil {
		t.Fatalf("yaml.Marshal: %v", err)
	}
	got = bytes.TrimPrefix(got, []byte("---\n"))

	assertGolden(t, "values.golden.yaml", got)
}

func deepLiteralizeForTest(v any) any {
	switch x := v.(type) {
	case string:
		if strings.Contains(x, "\n") {
			return literalString(x)
		}
		return x
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[k] = deepLiteralizeForTest(vv)
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = deepLiteralizeForTest(x[i])
		}
		return out
	default:
		return v
	}
}

func assertGolden(t *testing.T, name string, got []byte) {
	t.Helper()
	goldenPath := filepath.Join("testdata", "golden", name)
	update := os.Getenv("UPDATE_GOLDEN") == "1"

	if update {
		if err := os.MkdirAll(filepath.Dir(goldenPath), 0o755); err != nil {
			t.Fatalf("mkdir golden dir: %v", err)
		}
		if err := os.WriteFile(goldenPath, got, 0o644); err != nil {
			t.Fatalf("write golden: %v", err)
		}
	}

	want, err := os.ReadFile(goldenPath)
	if err != nil {
		t.Fatalf("read golden %s: %v (set UPDATE_GOLDEN=1 to create)", goldenPath, err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("golden mismatch for %s\n--- got ---\n%s\n--- want ---\n%s", name, string(got), string(want))
	}
}
