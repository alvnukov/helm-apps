package composeinspect

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLoad_BuildsComposeGraphRelations(t *testing.T) {
	p := filepath.Join("testdata", "compose-sample.yml")
	rep, err := Load(p)
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}
	if rep.Project != "sample-stack" {
		t.Fatalf("unexpected project: %q", rep.Project)
	}
	if rep.Summary.ServiceCount != 4 {
		t.Fatalf("expected 4 services, got %d", rep.Summary.ServiceCount)
	}
	mustRel := func(typ, from, to string) {
		t.Helper()
		for _, r := range rep.Relations {
			if r.Type == typ && r.From == from && r.To == to {
				return
			}
		}
		t.Fatalf("missing relation %s %s -> %s", typ, from, to)
	}
	mustRel("depends_on", "service:web", "service:db")
	mustRel("depends_on", "service:web", "service:redis")
	mustRel("env_ref", "service:web", "service:db")
	mustRel("env_ref", "service:web", "service:redis")
	mustRel("mounts_volume", "service:web", "volume:web-data")
	mustRel("uses_network", "service:web", "network:frontend")
	mustRel("uses_network", "service:web", "network:backend")
	mustRel("uses_config", "service:web", "config:app-config")
	mustRel("uses_secret", "service:web", "secret:app-secret")

	var web ServiceNode
	found := false
	for _, s := range rep.Services {
		if s.Name == "web" {
			web = s
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("web service node not found")
	}
	if len(web.PortsPublished) != 1 || web.PortsPublished[0].Raw != "8080:80" {
		t.Fatalf("unexpected ports: %#v", web.PortsPublished)
	}
	if len(web.RoleHints) == 0 {
		t.Fatalf("expected role hints for web")
	}
}

func TestResolveComposePath_DirectoryCandidates(t *testing.T) {
	dir := t.TempDir()
	if _, err := resolveComposePath(dir); err == nil {
		t.Fatalf("expected error when no compose file exists")
	}
	p := filepath.Join(dir, "docker-compose.yml")
	if err := os.WriteFile(p, []byte("services:\n  x:\n    image: nginx\n"), 0o644); err != nil {
		t.Fatalf("write compose file: %v", err)
	}
	got, err := resolveComposePath(dir)
	if err != nil {
		t.Fatalf("resolveComposePath error: %v", err)
	}
	if got != p {
		t.Fatalf("expected %s, got %s", p, got)
	}
}

func TestResolveAndWrite_FileOutputsAndBadFormat(t *testing.T) {
	src := filepath.Join("testdata", "compose-sample.yml")
	outDir := t.TempDir()
	jsonPath := filepath.Join(outDir, "report.json")
	if err := ResolveAndWrite(src, "json", jsonPath); err != nil {
		t.Fatalf("ResolveAndWrite json: %v", err)
	}
	if b, err := os.ReadFile(jsonPath); err != nil || !strings.Contains(string(b), `"services"`) {
		t.Fatalf("unexpected json file content err=%v", err)
	}
	yamlPath := filepath.Join(outDir, "report.yaml")
	if err := ResolveAndWrite(src, "yaml", yamlPath); err != nil {
		t.Fatalf("ResolveAndWrite yaml: %v", err)
	}
	if b, err := os.ReadFile(yamlPath); err != nil || !strings.Contains(string(b), "services:") {
		t.Fatalf("unexpected yaml file content err=%v", err)
	}
	if err := ResolveAndWrite(src, "xml", filepath.Join(outDir, "bad.out")); err == nil || !strings.Contains(err.Error(), "unsupported format") {
		t.Fatalf("expected unsupported format error, got %v", err)
	}
}

func TestParseHealthcheck_FormsAndDurations(t *testing.T) {
	h := parseHealthcheck(map[string]any{
		"test":         []any{"CMD-SHELL", "curl -f http://localhost || exit 1"},
		"interval":     "15s",
		"timeout":      "3s",
		"start_period": "1s",
		"retries":      5,
	})
	if h == nil || h.TestShell == "" {
		t.Fatalf("expected shell healthcheck, got %#v", h)
	}
	if h.IntervalSeconds != 15 || h.TimeoutSeconds != 3 || h.StartPeriodSeconds != 1 || h.Retries != 5 {
		t.Fatalf("unexpected timings: %#v", h)
	}

	h = parseHealthcheck(map[string]any{"test": []any{"CMD", "pg_isready", "-q"}})
	if h == nil || len(h.Test) != 2 || h.Test[0] != "pg_isready" {
		t.Fatalf("expected CMD healthcheck parsed, got %#v", h)
	}

	h = parseHealthcheck(map[string]any{"test": "NONE"})
	if h == nil || !h.Disable {
		t.Fatalf("expected disable from NONE, got %#v", h)
	}

	h = parseHealthcheck(map[string]any{"disable": true})
	if h == nil || !h.Disable {
		t.Fatalf("expected disable from disable:true, got %#v", h)
	}
}

func TestParseComposeDurationSeconds_AndHelpers(t *testing.T) {
	cases := []struct {
		in   any
		want int
	}{
		{"", 0},
		{"10", 10},
		{"1500ms", 1},
		{"5s", 5},
		{"bad", 0},
		{3, 3},
	}
	for _, tc := range cases {
		if got := parseComposeDurationSeconds(tc.in); got != tc.want {
			t.Fatalf("parseComposeDurationSeconds(%#v)=%d want %d", tc.in, got, tc.want)
		}
	}
	if list, shell := parseCommandLike([]any{"a", "b"}); len(list) != 2 || shell != "" {
		t.Fatalf("unexpected parseCommandLike(list): list=%#v shell=%q", list, shell)
	}
	if list, shell := parseCommandLike("echo hi"); list != nil || shell != "echo hi" {
		t.Fatalf("unexpected parseCommandLike(string): list=%#v shell=%q", list, shell)
	}
	if got := stringifyScalar(map[string]any{"a": 1}); !strings.Contains(got, `"a":1`) {
		t.Fatalf("expected json stringify for map, got %q", got)
	}
	for _, v := range []any{true, "true", "1", "yes", "on"} {
		if !isTrue(v) {
			t.Fatalf("expected isTrue(%#v)=true", v)
		}
	}
	if isTrue("no") {
		t.Fatalf("expected isTrue(\"no\")=false")
	}
}
