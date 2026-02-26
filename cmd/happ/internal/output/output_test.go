package output

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRendererTemplate_Golden(t *testing.T) {
	got := []byte(RendererTemplate("imported-raw-manifest"))
	assertGolden(t, "renderer.golden.tpl", got)
}

func TestValuesYAML_GlobalFirst(t *testing.T) {
	got, err := ValuesYAML(map[string]any{
		"apps-stateless": map[string]any{"demo": map[string]any{"enabled": true}},
		"global":         map[string]any{"env": "dev"},
	})
	if err != nil {
		t.Fatalf("ValuesYAML error: %v", err)
	}
	s := string(got)
	if !strings.HasPrefix(s, "global:\n") {
		t.Fatalf("expected global first, got:\n%s", s)
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
