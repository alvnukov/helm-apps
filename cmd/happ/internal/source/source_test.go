package source

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

func TestParseDocuments_ParsesAndFlattensList(t *testing.T) {
	data := []byte(`
apiVersion: v1
kind: ConfigMap
metadata:
  name: a
---
apiVersion: v1
kind: List
items:
  - apiVersion: v1
    kind: Secret
    metadata:
      name: s1
`)
	docs, err := ParseDocuments(data)
	if err != nil {
		t.Fatalf("ParseDocuments error: %v", err)
	}
	if len(docs) != 2 {
		t.Fatalf("expected 2 docs after flatten, got %d", len(docs))
	}
	if docs[0]["kind"] != "ConfigMap" || docs[1]["kind"] != "Secret" {
		t.Fatalf("unexpected docs: %#v", docs)
	}
}

func TestParseYAMLStream_InvalidYAML(t *testing.T) {
	if _, err := parseYAMLStream([]byte("a: [\n")); err == nil {
		t.Fatalf("expected parseYAMLStream error")
	}
}

func TestNormalizeYAML_ConvertsMapAnyKeys(t *testing.T) {
	in := map[any]any{"a": map[any]any{1: "x"}}
	out := normalizeYAML(in).(map[string]any)
	a := out["a"].(map[string]any)
	if a["1"] != "x" {
		t.Fatalf("expected map[any]any keys converted to string, got %#v", out)
	}
}

func TestCollectManifestFiles_SortsAndFiltersYAML(t *testing.T) {
	dir := t.TempDir()
	for _, p := range []string{"b.yaml", "a.yml", "x.txt"} {
		if err := os.WriteFile(filepath.Join(dir, p), []byte("x"), 0o644); err != nil {
			t.Fatalf("write file %s: %v", p, err)
		}
	}
	files, err := collectManifestFiles(dir)
	if err != nil {
		t.Fatalf("collectManifestFiles error: %v", err)
	}
	want := []string{filepath.Join(dir, "a.yml"), filepath.Join(dir, "b.yaml")}
	if !reflect.DeepEqual(files, want) {
		t.Fatalf("unexpected files: got %#v want %#v", files, want)
	}
}

func TestLoadManifestDocs_InvalidYAMLInFile(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "bad.yaml")
	if err := os.WriteFile(p, []byte("a: [\n"), 0o644); err != nil {
		t.Fatalf("write bad yaml: %v", err)
	}
	_, err := loadManifestDocs(dir)
	if err == nil || !strings.Contains(err.Error(), "invalid YAML in") {
		t.Fatalf("expected invalid YAML error, got %v", err)
	}
}

func TestLoadDocuments_ManifestsMode(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "a.yaml"), []byte("apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: a\n"), 0o644); err != nil {
		t.Fatalf("write yaml: %v", err)
	}
	docs, err := LoadDocuments(config.Config{SourceMode: "manifests", Input: dir})
	if err != nil {
		t.Fatalf("LoadDocuments error: %v", err)
	}
	if len(docs) != 1 || docs[0]["kind"] != "ConfigMap" {
		t.Fatalf("unexpected docs: %#v", docs)
	}
}

func TestLoadDocuments_ChartMode_UsesRenderedHelmOutput(t *testing.T) {
	orig := runHelmCommand
	defer func() { runHelmCommand = orig }()
	runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
		return []byte("apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: from-chart\n"), nil, nil
	}
	docs, err := LoadDocuments(config.Config{
		SourceMode:  "chart",
		Input:       "/chart",
		ReleaseName: "rel",
	})
	if err != nil {
		t.Fatalf("LoadDocuments(chart) error: %v", err)
	}
	if len(docs) != 1 || docs[0]["kind"] != "ConfigMap" {
		t.Fatalf("unexpected docs: %#v", docs)
	}
}

func TestRenderChartBytes_Wrapper(t *testing.T) {
	orig := runHelmCommand
	defer func() { runHelmCommand = orig }()
	runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
		return []byte("ok"), nil, nil
	}
	b, err := RenderChartBytes(config.Config{Input: "/chart", ReleaseName: "rel"}, "/chart")
	if err != nil {
		t.Fatalf("RenderChartBytes error: %v", err)
	}
	if string(b) != "ok" {
		t.Fatalf("unexpected bytes: %q", string(b))
	}
}

func TestRenderChartBytes_BuildsHelmArgsAndWritesOutput(t *testing.T) {
	orig := runHelmCommand
	defer func() { runHelmCommand = orig }()
	var gotBin string
	var gotArgs []string
	runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
		gotBin = bin
		gotArgs = append([]string{}, args...)
		return []byte("kind: ConfigMap\n"), nil, nil
	}
	outFile := filepath.Join(t.TempDir(), "rendered.yaml")
	cfg := config.Config{
		Input:           "/chart",
		SourceMode:      "chart",
		ReleaseName:     "rel",
		Namespace:       "ns",
		ValuesFiles:     []string{"v1.yaml"},
		SetValues:       []string{"a=b"},
		SetStringValues: []string{"c=d"},
		SetFileValues:   []string{"f=path"},
		SetJSONValues:   []string{"j={}"},
		KubeVersion:     "1.29.0",
		APIVersions:     []string{"v1", "apps/v1"},
		IncludeCRDs:     true,
		RenderedOutput:  outFile,
	}
	b, err := renderChartBytes(cfg, "/override-chart", true)
	if err != nil {
		t.Fatalf("renderChartBytes error: %v", err)
	}
	if string(b) != "kind: ConfigMap\n" {
		t.Fatalf("unexpected stdout: %q", string(b))
	}
	if gotBin != "helm" {
		t.Fatalf("expected helm binary, got %q", gotBin)
	}
	joined := strings.Join(gotArgs, " ")
	for _, part := range []string{"template rel /override-chart", "--namespace ns", "--values v1.yaml", "--set a=b", "--set-string c=d", "--set-file f=path", "--set-json j={}", "--kube-version 1.29.0", "--api-versions v1", "--api-versions apps/v1", "--include-crds"} {
		if !strings.Contains(joined, part) {
			t.Fatalf("expected args to contain %q, got %q", part, joined)
		}
	}
	if bb, err := os.ReadFile(outFile); err != nil || string(bb) != "kind: ConfigMap\n" {
		t.Fatalf("expected rendered output file, err=%v content=%q", err, string(bb))
	}
}

func TestRenderChartBytes_ErrorPaths(t *testing.T) {
	t.Run("timeout", func(t *testing.T) {
		orig := runHelmCommand
		defer func() { runHelmCommand = orig }()
		runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
			<-ctx.Done()
			return nil, nil, ctx.Err()
		}
		_, err := renderChartBytes(config.Config{Input: "/c", ReleaseName: "r", HelmExecTimeout: 5 * time.Millisecond}, "/c", false)
		if err == nil || !strings.Contains(err.Error(), "timed out") {
			t.Fatalf("expected timeout error, got %v", err)
		}
	})
	t.Run("helm-not-found", func(t *testing.T) {
		orig := runHelmCommand
		defer func() { runHelmCommand = orig }()
		runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
			return nil, nil, exec.ErrNotFound
		}
		_, err := renderChartBytes(config.Config{Input: "/c", ReleaseName: "r"}, "/c", false)
		if err == nil || !strings.Contains(err.Error(), "helm binary not found") {
			t.Fatalf("expected helm not found error, got %v", err)
		}
	})
	t.Run("stderr-message", func(t *testing.T) {
		orig := runHelmCommand
		defer func() { runHelmCommand = orig }()
		runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
			return nil, []byte("boom stderr"), errors.New("exit")
		}
		_, err := renderChartBytes(config.Config{Input: "/c", ReleaseName: "r"}, "/c", false)
		if err == nil || !strings.Contains(err.Error(), "boom stderr") {
			t.Fatalf("expected stderr in error, got %v", err)
		}
	})
	t.Run("write-rendered-output-fails", func(t *testing.T) {
		orig := runHelmCommand
		defer func() { runHelmCommand = orig }()
		runHelmCommand = func(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
			return []byte("x"), nil, nil
		}
		_, err := renderChartBytes(config.Config{
			Input:          "/c",
			ReleaseName:    "r",
			RenderedOutput: filepath.Join(t.TempDir(), "missing", "rendered.yaml"),
		}, "/c", true)
		if err == nil {
			t.Fatalf("expected write rendered output error")
		}
	})
}

func TestLoadManifestDocs_NoYAMLFiles(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "a.txt"), []byte("x"), 0o644); err != nil {
		t.Fatalf("write txt: %v", err)
	}
	_, err := loadManifestDocs(dir)
	if err == nil || !strings.Contains(err.Error(), "no YAML files found") {
		t.Fatalf("expected no YAML files error, got %v", err)
	}
}

func TestCollectManifestFiles_FilePathAndMissingPath(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "one.yaml")
	if err := os.WriteFile(p, []byte("x"), 0o644); err != nil {
		t.Fatalf("write yaml: %v", err)
	}
	files, err := collectManifestFiles(p)
	if err != nil {
		t.Fatalf("collectManifestFiles(file) error: %v", err)
	}
	if !reflect.DeepEqual(files, []string{p}) {
		t.Fatalf("unexpected files for single path: %#v", files)
	}
	_, err = collectManifestFiles(filepath.Join(dir, "missing"))
	if err == nil {
		t.Fatalf("expected stat error for missing path")
	}
}

func TestDefaultHelmCommandRunner(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("shell invocation differs on windows")
	}
	stdout, stderr, err := defaultHelmCommandRunner(context.Background(), "sh", []string{"-c", "printf out; printf err >&2"})
	if err != nil {
		t.Fatalf("defaultHelmCommandRunner error: %v", err)
	}
	if string(stdout) != "out" || string(stderr) != "err" {
		t.Fatalf("unexpected stdio stdout=%q stderr=%q", string(stdout), string(stderr))
	}
}
