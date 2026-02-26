package output

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

func TestWriteRendererTemplate_SkipsForNativeFallback(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "x.tpl")
	if err := WriteRendererTemplate(path, "apps-k8s-manifests"); err != nil {
		t.Fatalf("WriteRendererTemplate error: %v", err)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected no file created, got err=%v", err)
	}
}

func TestWriteRendererTemplate_WritesCustomRenderer(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "x.tpl")
	if err := WriteRendererTemplate(path, "custom-group"); err != nil {
		t.Fatalf("WriteRendererTemplate error: %v", err)
	}
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read file: %v", err)
	}
	if !strings.Contains(string(b), `define "custom-group.render"`) {
		t.Fatalf("unexpected renderer template content: %s", string(b))
	}
}

func TestGenerateConsumerChart_NativeFallbackDoesNotCreateRenderer(t *testing.T) {
	outDir := t.TempDir()
	values := map[string]any{
		"global": map[string]any{"env": "dev"},
		"apps-k8s-manifests": map[string]any{
			"x": map[string]any{"enabled": true, "apiVersion": "v1", "kind": "ConfigMap", "name": "x"},
		},
	}
	if err := GenerateConsumerChart(outDir, "demo", "apps-k8s-manifests", values, ""); err != nil {
		t.Fatalf("GenerateConsumerChart: %v", err)
	}
	if _, err := os.Stat(filepath.Join(outDir, "templates", "zz-imported-renderer.tpl")); !os.IsNotExist(err) {
		t.Fatalf("expected no renderer template for native fallback, got err=%v", err)
	}
}

func TestGenerateConsumerChart_CustomRendererCreatesTemplate(t *testing.T) {
	outDir := t.TempDir()
	values := map[string]any{
		"global": map[string]any{"env": "dev"},
		"custom-group": map[string]any{
			"__GroupVars__": map[string]any{"type": "my-renderer"},
			"x":             map[string]any{"enabled": true},
		},
	}
	if err := GenerateConsumerChart(outDir, "demo", "my-renderer", values, ""); err != nil {
		t.Fatalf("GenerateConsumerChart: %v", err)
	}
	b, err := os.ReadFile(filepath.Join(outDir, "templates", "zz-imported-renderer.tpl"))
	if err != nil {
		t.Fatalf("read renderer template: %v", err)
	}
	if !strings.Contains(string(b), `define "my-renderer.render"`) {
		t.Fatalf("unexpected renderer template content")
	}
}

func TestGenerateConsumerChart_CopiesLibraryChart(t *testing.T) {
	libDir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(libDir, "templates"), 0o755); err != nil {
		t.Fatalf("mkdir lib templates: %v", err)
	}
	if err := os.WriteFile(filepath.Join(libDir, "Chart.yaml"), []byte("apiVersion: v2\nname: helm-apps\nversion: 0.0.1\n"), 0o644); err != nil {
		t.Fatalf("write lib chart: %v", err)
	}
	if err := os.WriteFile(filepath.Join(libDir, "templates", "x.yaml"), []byte("kind: ConfigMap\n"), 0o644); err != nil {
		t.Fatalf("write lib template: %v", err)
	}
	outDir := t.TempDir()
	values := map[string]any{"global": map[string]any{"env": "dev"}, "apps-k8s-manifests": map[string]any{}}
	if err := GenerateConsumerChart(outDir, "demo", "apps-k8s-manifests", values, libDir); err != nil {
		t.Fatalf("GenerateConsumerChart: %v", err)
	}
	if _, err := os.Stat(filepath.Join(outDir, "charts", "helm-apps", "Chart.yaml")); err != nil {
		t.Fatalf("expected vendored Chart.yaml: %v", err)
	}
	if _, err := os.Stat(filepath.Join(outDir, "charts", "helm-apps", "templates", "x.yaml")); err != nil {
		t.Fatalf("expected vendored template: %v", err)
	}
}

func TestLiteralStringMarshalYAML_AndDeepLiteralize(t *testing.T) {
	n, err := literalString("a: 1\nb: 2").MarshalYAML()
	if err != nil {
		t.Fatalf("MarshalYAML error: %v", err)
	}
	node, ok := n.(*yaml.Node)
	if !ok || node.Style != yaml.LiteralStyle {
		t.Fatalf("expected literal yaml node, got %#v", n)
	}

	v := deepLiteralize(map[string]any{
		"a": "single line",
		"b": "multi\nline",
		"c": []any{"x\ny"},
	}).(map[string]any)
	if _, ok := v["a"].(literalString); ok {
		t.Fatalf("single-line string should not be literalString")
	}
	if _, ok := v["b"].(literalString); !ok {
		t.Fatalf("multi-line string should be literalString")
	}
	arr := v["c"].([]any)
	if _, ok := arr[0].(literalString); !ok {
		t.Fatalf("nested multiline string should be literalString")
	}
}

func TestWriteValues_WritesToFileAndStdout(t *testing.T) {
	values := map[string]any{"global": map[string]any{"env": "dev"}}
	outFile := filepath.Join(t.TempDir(), "values.yaml")
	if err := WriteValues(outFile, values); err != nil {
		t.Fatalf("WriteValues(file) error: %v", err)
	}
	if b, err := os.ReadFile(outFile); err != nil || !bytes.Contains(b, []byte("global:")) {
		t.Fatalf("unexpected file output err=%v body=%q", err, string(b))
	}

	orig := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("os.Pipe: %v", err)
	}
	os.Stdout = w
	defer func() { os.Stdout = orig }()
	if err := WriteValues("", values); err != nil {
		t.Fatalf("WriteValues(stdout) error: %v", err)
	}
	_ = w.Close()
	b, _ := io.ReadAll(r)
	if !bytes.Contains(b, []byte("global:")) {
		t.Fatalf("unexpected stdout output: %q", string(b))
	}
}

func TestValuesYAML_DeepLiteralizeAndTrimDocumentPrefix(t *testing.T) {
	b, err := ValuesYAML(map[string]any{
		"apps-k8s-manifests": map[string]any{
			"x": map[string]any{"enabled": true, "spec": "line1\nline2\n"},
		},
		"global": map[string]any{"env": "dev"},
	})
	if err != nil {
		t.Fatalf("ValuesYAML error: %v", err)
	}
	s := string(b)
	if strings.HasPrefix(s, "---\n") {
		t.Fatalf("expected yaml doc prefix trimmed, got:\n%s", s)
	}
	if !strings.Contains(s, "spec: |-") && !strings.Contains(s, "spec: |") {
		t.Fatalf("expected literal block string for multiline text, got:\n%s", s)
	}
}

func TestReorderTopLevelGlobalFirstYAML_EdgeCases(t *testing.T) {
	// invalid yaml
	if _, err := reorderTopLevelGlobalFirstYAML([]byte("a: [\n")); err == nil {
		t.Fatalf("expected error for invalid yaml")
	}
	// non-map doc should pass through unchanged
	in := []byte("- a\n- b\n")
	out, err := reorderTopLevelGlobalFirstYAML(in)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if string(out) != string(in) {
		t.Fatalf("expected unchanged non-map yaml, got %q", string(out))
	}
	// no global should remain same order
	in = []byte("a: 1\nb: 2\n")
	out, err = reorderTopLevelGlobalFirstYAML(in)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(string(out), "a: 1") {
		t.Fatalf("unexpected output: %q", string(out))
	}
}
