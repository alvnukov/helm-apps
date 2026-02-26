package composeimport

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/zol/helm-apps/cmd/happ/internal/composeinspect"
	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

func TestBuildValues_FromComposeReport_AppCentricPorts(t *testing.T) {
	rep, err := composeinspect.Load(filepath.Join("..", "composeinspect", "testdata", "compose-sample.yml"))
	if err != nil {
		t.Fatalf("load compose report: %v", err)
	}
	values, warnings, err := BuildValues(config.Config{Env: "dev"}, rep)
	if err != nil {
		t.Fatalf("BuildValues error: %v", err)
	}
	if _, ok := values["apps-stateless"]; !ok {
		t.Fatalf("expected apps-stateless, got %#v", values)
	}
	apps := values["apps-stateless"].(map[string]any)
	web := apps["web"].(map[string]any)
	if _, ok := web["service"]; !ok {
		t.Fatalf("expected web.service from compose ports")
	}
	containers := web["containers"].(map[string]any)
	main := containers["main"].(map[string]any)
	if _, ok := main["ports"]; !ok {
		t.Fatalf("expected container ports for web")
	}
	if _, ok := values["apps-services"]; ok {
		t.Fatalf("did not expect standalone apps-services in app-centric MVP")
	}
	_ = apps["worker"].(map[string]any)
	foundProfileWarn := false
	for _, w := range warnings {
		if strings.Contains(w, "profiles") {
			foundProfileWarn = true
			break
		}
	}
	if !foundProfileWarn {
		t.Fatalf("expected profile warning, got %#v", warnings)
	}
}

func TestBuildValues_MapsCommandEntrypointAndWorkingDir(t *testing.T) {
	rep := composeinspect.Report{
		Services: []composeinspect.ServiceNode{
			{
				ID:         "service:web",
				Name:       "web",
				Image:      "nginx:1.25",
				Entrypoint: []string{"/docker-entrypoint.sh"},
				Command:    []string{"nginx", "-g", "daemon off;"},
				WorkingDir: "/app",
			},
		},
	}
	values, _, err := BuildValues(config.Config{Env: "dev"}, rep)
	if err != nil {
		t.Fatalf("BuildValues error: %v", err)
	}
	apps := values["apps-stateless"].(map[string]any)
	web := apps["web"].(map[string]any)
	main := web["containers"].(map[string]any)["main"].(map[string]any)
	if _, ok := main["command"]; !ok {
		t.Fatalf("expected command from compose entrypoint")
	}
	if _, ok := main["args"]; !ok {
		t.Fatalf("expected args from compose command")
	}
	if got, _ := main["workingDir"].(string); got != "/app" {
		t.Fatalf("expected workingDir=/app, got %#v", main["workingDir"])
	}
}

func TestBuildValues_MapsHealthcheckToReadinessProbe(t *testing.T) {
	rep := composeinspect.Report{
		Services: []composeinspect.ServiceNode{
			{
				ID:    "service:db",
				Name:  "db",
				Image: "postgres:16",
				Healthcheck: &composeinspect.Healthcheck{
					Test:               []string{"pg_isready", "-q"},
					IntervalSeconds:    10,
					TimeoutSeconds:     3,
					StartPeriodSeconds: 20,
					Retries:            5,
				},
			},
		},
	}
	values, _, err := BuildValues(config.Config{Env: "dev"}, rep)
	if err != nil {
		t.Fatalf("BuildValues error: %v", err)
	}
	main := values["apps-stateless"].(map[string]any)["db"].(map[string]any)["containers"].(map[string]any)["main"].(map[string]any)
	probe, ok := main["readinessProbe"].(map[string]any)
	if !ok {
		t.Fatalf("expected readinessProbe map, got %#v", main["readinessProbe"])
	}
	execMap, _ := probe["exec"].(map[string]any)
	cmd, _ := execMap["command"].([]any)
	if len(cmd) != 2 || cmd[0] != "pg_isready" {
		t.Fatalf("unexpected readinessProbe.exec.command: %#v", cmd)
	}
	if got := probe["periodSeconds"]; got != 10 {
		t.Fatalf("expected periodSeconds=10, got %#v", got)
	}
	if got := probe["initialDelaySeconds"]; got != 20 {
		t.Fatalf("expected initialDelaySeconds=20, got %#v", got)
	}
}

func TestBuildValues_HealthcheckOptInLiveness(t *testing.T) {
	rep := composeinspect.Report{
		Services: []composeinspect.ServiceNode{{
			ID:    "service:web",
			Name:  "web",
			Image: "nginx:1.25",
			Healthcheck: &composeinspect.Healthcheck{
				TestShell: "curl -f http://localhost || exit 1",
			},
		}},
	}
	values, warnings, err := BuildValues(config.Config{Env: "dev", ComposeHealthcheckToLiveness: true}, rep)
	if err != nil {
		t.Fatalf("BuildValues error: %v", err)
	}
	main := values["apps-stateless"].(map[string]any)["web"].(map[string]any)["containers"].(map[string]any)["main"].(map[string]any)
	if _, ok := main["readinessProbe"]; !ok {
		t.Fatalf("expected readinessProbe")
	}
	if _, ok := main["livenessProbe"]; !ok {
		t.Fatalf("expected livenessProbe with opt-in flag")
	}
	hasWarn := false
	for _, w := range warnings {
		if strings.Contains(w, "livenessProbe") {
			hasWarn = true
			break
		}
	}
	if !hasWarn {
		t.Fatalf("expected warning about livenessProbe opt-in, got %#v", warnings)
	}
}

func TestBuildValues_NoMappableServicesReturnsError(t *testing.T) {
	rep := composeinspect.Report{
		Services: []composeinspect.ServiceNode{{
			ID:       "service:x",
			Name:     "x",
			HasBuild: true,
			Image:    "",
		}},
	}
	_, warnings, err := BuildValues(config.Config{Env: "dev"}, rep)
	if err == nil || !strings.Contains(err.Error(), "no compose services could be mapped") {
		t.Fatalf("expected no mappable services error, got %v", err)
	}
	if len(warnings) == 0 {
		t.Fatalf("expected warnings for unsupported image/build")
	}
}

func TestPortHelpersAndParsers(t *testing.T) {
	if p, proto := parseExpose("8080/udp"); p != "8080" || proto != "udp" {
		t.Fatalf("unexpected parseExpose result: %q %q", p, proto)
	}
	if got := defaultPortName("80", 1); got != "http" {
		t.Fatalf("expected http, got %q", got)
	}
	if got := defaultPortName("443", 1); got != "https" {
		t.Fatalf("expected https, got %q", got)
	}
	if got := defaultPortName("9090", 1); got == "" {
		t.Fatalf("expected generated port name")
	}
	if target, published, proto, ok := parseComposePortBinding(composeinspect.PortBinding{Raw: "127.0.0.1:15432:5432/tcp"}); !ok || target != "5432" || published != "15432" || proto != "tcp" {
		t.Fatalf("unexpected short port parse: target=%q published=%q proto=%q ok=%v", target, published, proto, ok)
	}
	if target, published, proto, ok := parseComposePortBinding(composeinspect.PortBinding{Target: "8080", Published: "80", Protocol: "udp"}); !ok || target != "8080" || published != "80" || proto != "udp" {
		t.Fatalf("unexpected long port parse: target=%q published=%q proto=%q ok=%v", target, published, proto, ok)
	}
	if _, _, _, ok := parseComposePortBinding(composeinspect.PortBinding{}); ok {
		t.Fatalf("expected empty port binding to fail")
	}
}

func TestBuildPorts_DedupesAndWarnsUnsupported(t *testing.T) {
	s := composeinspect.ServiceNode{
		Name: "web",
		PortsPublished: []composeinspect.PortBinding{
			{Raw: "8080:80"},
			{Raw: "8080:80"},
			{Raw: ""},
		},
		Expose: []string{"80", "9090/udp"},
	}
	cports, svcPorts, warns := buildPorts(s)
	if len(cports) == 0 || len(svcPorts) == 0 {
		t.Fatalf("expected ports to be generated")
	}
	if len(warns) == 0 {
		t.Fatalf("expected warning for unsupported empty binding")
	}
}

func TestCloneAndHelpers(t *testing.T) {
	src := map[string]any{
		"exec": map[string]any{
			"command": []any{"a", "b"},
		},
	}
	cl := cloneMapAny(src)
	cl["exec"].(map[string]any)["command"].([]any)[0] = "x"
	if src["exec"].(map[string]any)["command"].([]any)[0] != "a" {
		t.Fatalf("expected deep clone to avoid mutating source")
	}
	if img, ok := parseImage("repo/app"); !ok || img["staticTag"] != "latest" {
		t.Fatalf("expected latest tag fallback, got %#v ok=%v", img, ok)
	}
	if k := dedupeKey(map[string]any{"svc": map[string]any{}, "svc-2": map[string]any{}}, "svc"); k == "svc" {
		t.Fatalf("expected dedupeKey to avoid collision, got %q", k)
	}
}
