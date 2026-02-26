package app

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/analyze"
	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

func TestServiceRun_GeneratesConsumerChartFromManifests(t *testing.T) {
	tmp := t.TempDir()
	manifestsPath := filepath.Join(tmp, "in.yaml")
	outChart := filepath.Join(tmp, "out-chart")

	manifest := `apiVersion: v1
kind: ConfigMap
metadata:
  name: demo
  labels:
    team: platform
data:
  A: "1"
---
apiVersion: v1
kind: Secret
metadata:
  name: demo-secret
type: Opaque
stringData:
  token: abc
`
	if err := os.WriteFile(manifestsPath, []byte(manifest), 0o644); err != nil {
		t.Fatal(err)
	}

	cfg := config.Config{
		SourceMode:        "manifests",
		Input:             manifestsPath,
		Env:               "dev",
		GroupName:         config.DefaultGroupName,
		GroupType:         config.DefaultGroupType,
		MinIncludeBytes:   1,
		OutChartDir:       outChart,
		ConsumerChartName: "demo-imported",
	}

	if err := (Service{}).Run(cfg); err != nil {
		t.Fatalf("Run returned error: %v", err)
	}

	required := []string{
		filepath.Join(outChart, "Chart.yaml"),
		filepath.Join(outChart, "values.yaml"),
		filepath.Join(outChart, "templates", "init-helm-apps-library.yaml"),
	}
	for _, p := range required {
		if _, err := os.Stat(p); err != nil {
			t.Fatalf("expected generated file %s: %v", p, err)
		}
	}

	valuesData, err := os.ReadFile(filepath.Join(outChart, "values.yaml"))
	if err != nil {
		t.Fatal(err)
	}
	if len(valuesData) == 0 {
		t.Fatalf("generated values.yaml is empty")
	}
}

func TestServiceRun_HelpersExperimental_MapsSimpleConfigMapAndSecret(t *testing.T) {
	tmp := t.TempDir()
	manifestsPath := filepath.Join(tmp, "in.yaml")
	outChart := filepath.Join(tmp, "out-chart")
	manifest := `apiVersion: v1
kind: ConfigMap
metadata:
  name: cfg
data:
  A: "1"
---
apiVersion: v1
kind: Secret
metadata:
  name: sec
type: Opaque
data:
  token: YWJj
`
	if err := os.WriteFile(manifestsPath, []byte(manifest), 0o644); err != nil {
		t.Fatal(err)
	}
	cfg := config.Config{
		SourceMode:        "manifests",
		Input:             manifestsPath,
		Env:               "dev",
		GroupName:         config.DefaultGroupName,
		GroupType:         config.DefaultGroupType,
		MinIncludeBytes:   1,
		OutChartDir:       outChart,
		ConsumerChartName: "demo-imported",
		ImportStrategy:    config.ImportStrategyHelpersExperimental,
	}
	if err := (Service{}).Run(cfg); err != nil {
		t.Fatalf("Run returned error: %v", err)
	}
	valuesData, err := os.ReadFile(filepath.Join(outChart, "values.yaml"))
	if err != nil {
		t.Fatal(err)
	}
	s := string(valuesData)
	if !containsAll(s, "apps-configmaps:", "apps-secrets:") {
		t.Fatalf("expected helper groups in values.yaml, got:\n%s", s)
	}
	if containsAll(s, "apps-k8s-manifests:", "__GroupVars__") {
		t.Fatalf("did not expect legacy raw fallback group markers, got:\n%s", s)
	}
}

func containsAll(s string, parts ...string) bool {
	for _, p := range parts {
		if !strings.Contains(s, p) {
			return false
		}
	}
	return true
}

func TestServiceRun_ChartModeVerifyEquivalence(t *testing.T) {
	if _, err := exec.LookPath("helm"); err != nil {
		t.Skip("helm binary not available")
	}

	tmp := t.TempDir()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}

	fixtureChart := filepath.Clean(filepath.Join(wd, "..", "..", "testdata", "simple-chart"))
	libraryChart := filepath.Clean(filepath.Join(wd, "..", "..", "..", "..", "charts", "helm-apps"))
	outChart := filepath.Join(tmp, "generated-consumer-chart")

	cfg := config.Config{
		SourceMode:        "chart",
		Input:             fixtureChart,
		ReleaseName:       "sample",
		Env:               "dev",
		GroupName:         config.DefaultGroupName,
		GroupType:         config.DefaultGroupType,
		MinIncludeBytes:   1,
		OutChartDir:       outChart,
		ConsumerChartName: "sample-imported",
		LibraryChartPath:  libraryChart,
		VerifyEquivalence: true,
	}

	if err := (Service{}).Run(cfg); err != nil {
		t.Fatalf("chart mode Run returned error: %v", err)
	}

	for _, p := range []string{
		filepath.Join(outChart, "Chart.yaml"),
		filepath.Join(outChart, "values.yaml"),
		filepath.Join(outChart, "charts", "helm-apps", "Chart.yaml"),
	} {
		if _, err := os.Stat(p); err != nil {
			t.Fatalf("expected generated file %s: %v", p, err)
		}
	}
}

func TestRunWithTimeout(t *testing.T) {
	if err := runWithTimeout(0, "stage", func() error { return nil }); err != nil {
		t.Fatalf("expected nil without timeout, got %v", err)
	}
	wantErr := errors.New("boom")
	if err := runWithTimeout(20*time.Millisecond, "stage", func() error { return wantErr }); !errors.Is(err, wantErr) {
		t.Fatalf("expected original error, got %v", err)
	}
	err := runWithTimeout(10*time.Millisecond, "stage", func() error {
		time.Sleep(30 * time.Millisecond)
		return nil
	})
	if err == nil || !strings.Contains(err.Error(), "stage timed out") {
		t.Fatalf("expected timeout error, got %v", err)
	}
}

func TestCompareDocHelpers(t *testing.T) {
	doc := map[string]any{
		"apiVersion": "v1",
		"kind":       "ConfigMap",
		"metadata":   map[string]any{"name": "demo"},
		"data":       map[string]any{"a": "1"},
	}
	if got := compareDocKey(doc); got != "v1/ConfigMap/default/demo" {
		t.Fatalf("unexpected key: %q", got)
	}
	if got := compareDocKey(map[string]any{"apiVersion": "v1"}); got != "" {
		t.Fatalf("expected empty key, got %q", got)
	}
	if _, ok := docsByKey([]map[string]any{doc})["v1/ConfigMap/default/demo"]; !ok {
		t.Fatalf("docsByKey missing expected key")
	}
	yamls := docsYAMLByKey([]map[string]any{doc})
	if !containsAll(yamls["v1/ConfigMap/default/demo"], "apiVersion: v1", "kind: ConfigMap", "name: demo") {
		t.Fatalf("unexpected docsYAMLByKey output: %q", yamls["v1/ConfigMap/default/demo"])
	}
}

func TestLoadChartValuesYAML(t *testing.T) {
	dir := t.TempDir()
	if got := loadChartValuesYAML(dir); got != "" {
		t.Fatalf("expected empty string when values.yaml missing, got %q", got)
	}
	if err := os.WriteFile(filepath.Join(dir, "values.yaml"), []byte("a: 1\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if got := loadChartValuesYAML(dir); got != "a: 1\n" {
		t.Fatalf("unexpected values content: %q", got)
	}
}

func TestRunPipeline_DyffWritesOutputFile(t *testing.T) {
	tmp := t.TempDir()
	fromPath := filepath.Join(tmp, "from.yaml")
	toPath := filepath.Join(tmp, "to.yaml")
	outPath := filepath.Join(tmp, "diff.txt")
	if err := os.WriteFile(fromPath, []byte("a: 1\n"), 0o644); err != nil {
		t.Fatalf("write from: %v", err)
	}
	if err := os.WriteFile(toPath, []byte("a: 2\n"), 0o644); err != nil {
		t.Fatalf("write to: %v", err)
	}

	cfg := config.Config{
		Command:  config.CommandDyff,
		DyffFrom: fromPath,
		DyffTo:   toPath,
		Output:   outPath,
	}
	if err := runPipeline(cfg); err != nil {
		t.Fatalf("runPipeline dyff error: %v", err)
	}
	b, err := os.ReadFile(outPath)
	if err != nil {
		t.Fatalf("read diff: %v", err)
	}
	if !strings.Contains(string(b), "~ $.a: 1 -> 2") {
		t.Fatalf("unexpected diff output: %q", string(b))
	}
}

func TestRunPipeline_DyffFailOnDiff(t *testing.T) {
	tmp := t.TempDir()
	fromPath := filepath.Join(tmp, "from.yaml")
	toPath := filepath.Join(tmp, "to.yaml")
	if err := os.WriteFile(fromPath, []byte("a: 1\n"), 0o644); err != nil {
		t.Fatalf("write from: %v", err)
	}
	if err := os.WriteFile(toPath, []byte("a: 2\n"), 0o644); err != nil {
		t.Fatalf("write to: %v", err)
	}
	cfg := config.Config{
		Command:        config.CommandDyff,
		DyffFrom:       fromPath,
		DyffTo:         toPath,
		DyffQuiet:      true,
		DyffFailOnDiff: true,
	}
	err := runPipeline(cfg)
	if !errors.Is(err, errDyffDifferent) {
		t.Fatalf("expected errDyffDifferent, got %v", err)
	}
}

func TestRunPipeline_DyffNoDiffEmptyOutputFile(t *testing.T) {
	tmp := t.TempDir()
	fromPath := filepath.Join(tmp, "from.yaml")
	toPath := filepath.Join(tmp, "to.yaml")
	outPath := filepath.Join(tmp, "diff.txt")
	if err := os.WriteFile(fromPath, []byte("a: 1\n"), 0o644); err != nil {
		t.Fatalf("write from: %v", err)
	}
	if err := os.WriteFile(toPath, []byte("a: 1\n"), 0o644); err != nil {
		t.Fatalf("write to: %v", err)
	}
	cfg := config.Config{
		Command:        config.CommandDyff,
		DyffFrom:       fromPath,
		DyffTo:         toPath,
		Output:         outPath,
		DyffFailOnDiff: true,
	}
	if err := runPipeline(cfg); err != nil {
		t.Fatalf("runPipeline dyff equal error: %v", err)
	}
	b, err := os.ReadFile(outPath)
	if err != nil {
		t.Fatalf("read output: %v", err)
	}
	if strings.TrimSpace(string(b)) != "" {
		t.Fatalf("expected empty diff file for equal docs, got %q", string(b))
	}
}

func TestRunPipeline_DyffJSONOutput(t *testing.T) {
	tmp := t.TempDir()
	fromPath := filepath.Join(tmp, "from.yaml")
	toPath := filepath.Join(tmp, "to.yaml")
	outPath := filepath.Join(tmp, "diff.json")
	if err := os.WriteFile(fromPath, []byte("a: 1\n"), 0o644); err != nil {
		t.Fatalf("write from: %v", err)
	}
	if err := os.WriteFile(toPath, []byte("a: 2\n"), 0o644); err != nil {
		t.Fatalf("write to: %v", err)
	}
	cfg := config.Config{
		Command:     config.CommandDyff,
		DyffFrom:    fromPath,
		DyffTo:      toPath,
		Output:      outPath,
		DyffFormat:  "json",
		DyffQuiet:   true,
		DyffColor:   "never",
		DyffStats:   true,
		DyffLabelTo: "ignored-in-json",
	}
	if err := runPipeline(cfg); err != nil {
		t.Fatalf("runPipeline dyff json error: %v", err)
	}
	b, err := os.ReadFile(outPath)
	if err != nil {
		t.Fatalf("read json output: %v", err)
	}
	s := string(b)
	if !strings.Contains(s, "\"equal\": false") || !strings.Contains(s, "\"changed\": 1") {
		t.Fatalf("unexpected json output: %s", s)
	}
}

func TestBuildInspectModel_GeneratesPreview(t *testing.T) {
	cfg := config.Config{
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyRaw,
	}
	docs := []map[string]any{{
		"apiVersion": "v1",
		"kind":       "ConfigMap",
		"metadata":   map[string]any{"name": "demo"},
		"data":       map[string]any{"a": "1"},
	}}
	rep := &analyze.Report{
		ChartPath:     "chart",
		TemplateFiles: 1,
	}
	model := buildInspectModel(cfg, docs, rep)
	if model.Summary.ResourceCount != 1 {
		t.Fatalf("unexpected model summary: %+v", model.Summary)
	}
	if model.HelmApps == nil || model.HelmApps.Error != "" {
		t.Fatalf("expected preview without error, got %+v", model.HelmApps)
	}
	if !strings.Contains(model.HelmApps.ValuesYAML, "apps-k8s-manifests:") {
		t.Fatalf("expected apps-k8s-manifests preview, got:\n%s", model.HelmApps.ValuesYAML)
	}
}

func TestRunPipeline_InspectWithoutWebErrors(t *testing.T) {
	p := filepath.Join(t.TempDir(), "m.yaml")
	if err := os.WriteFile(p, []byte("apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: a\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	err := runPipeline(config.Config{
		Command:        config.CommandInspect,
		SourceMode:     "manifests",
		Input:          p,
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyRaw,
	})
	if err == nil || !strings.Contains(err.Error(), "supports only --web") {
		t.Fatalf("expected inspect --web error, got %v", err)
	}
}

func TestRunPipeline_VerifyEquivalenceValidationErrors(t *testing.T) {
	p := filepath.Join(t.TempDir(), "m.yaml")
	if err := os.WriteFile(p, []byte("apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: a\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	cfg := config.Config{
		Command:           config.CommandImport,
		SourceMode:        "manifests",
		Input:             p,
		Env:               "dev",
		GroupName:         config.DefaultGroupName,
		GroupType:         config.DefaultGroupType,
		ImportStrategy:    config.ImportStrategyRaw,
		VerifyEquivalence: true,
	}
	if err := runPipeline(cfg); err == nil || !strings.Contains(err.Error(), "requires chart mode") {
		t.Fatalf("expected chart-mode validation error, got %v", err)
	}

	if _, err := exec.LookPath("helm"); err != nil {
		t.Skip("helm binary not available for chart-mode validation path")
	}
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	fixtureChart := filepath.Clean(filepath.Join(wd, "..", "..", "testdata", "simple-chart"))
	cfg.SourceMode = "chart"
	cfg.Input = fixtureChart
	cfg.ReleaseName = "sample"
	if err := runPipeline(cfg); err == nil || !strings.Contains(err.Error(), "requires --out-chart-dir") {
		t.Fatalf("expected out-chart-dir validation error, got %v", err)
	}
}

func TestRunPipeline_ComposeModesNonWeb(t *testing.T) {
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	composePath := filepath.Clean(filepath.Join(wd, "..", "composeinspect", "testdata", "compose-sample.yml"))

	reportOut := filepath.Join(t.TempDir(), "compose-report.yaml")
	if err := runPipeline(config.Config{
		Command:       config.CommandComposeInspect,
		Input:         composePath,
		ComposeFormat: "yaml",
		Output:        reportOut,
	}); err != nil {
		t.Fatalf("compose-inspect non-web error: %v", err)
	}
	if _, err := os.Stat(reportOut); err != nil {
		t.Fatalf("compose inspect report not written: %v", err)
	}

	valuesOut := filepath.Join(t.TempDir(), "values.yaml")
	if err := runPipeline(config.Config{
		Command:        config.CommandImport,
		SourceMode:     "compose",
		Input:          composePath,
		Env:            "dev",
		GroupName:      config.DefaultGroupName,
		GroupType:      config.DefaultGroupType,
		ImportStrategy: config.ImportStrategyRaw,
		Output:         valuesOut,
	}); err != nil {
		t.Fatalf("compose import error: %v", err)
	}
	b, err := os.ReadFile(valuesOut)
	if err != nil {
		t.Fatalf("read values output: %v", err)
	}
	if !strings.Contains(string(b), "apps-stateless:") {
		t.Fatalf("expected compose import to produce apps-stateless, got:\n%s", string(b))
	}
}
