package cli

import (
	"bytes"
	"errors"
	"flag"
	"strings"
	"testing"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

func TestParse_RootHelp(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"--help"}, &out, &errOut)
	if !errors.Is(err, flag.ErrHelp) {
		t.Fatalf("expected flag.ErrHelp, got %v", err)
	}
	if !strings.Contains(out.String(), "happ") {
		t.Fatalf("expected usage in stdout, got %q", out.String())
	}
}

func TestParse_UnknownSubcommand(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"wat"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "unknown subcommand") {
		t.Fatalf("expected unknown subcommand error, got %v", err)
	}
	if errOut.Len() == 0 {
		t.Fatalf("expected usage in stderr")
	}
}

func TestParse_ChartHappyPath(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{
		"chart",
		"--path", "/tmp/chart",
		"--values", "a.yaml",
		"--values", "b.yaml",
		"--set", "image.tag=1",
		"--set-string", "x=y",
		"--set-file", "f=./file",
		"--set-json", "j={}",
		"--namespace", "ns1",
		"--release-name", "rel1",
		"--import-strategy", config.ImportStrategyHelpersExperimental,
		"--verify-equivalence",
		"--analyze-templates",
		"--api-version", "v1",
		"--api-version", "apps/v1",
		"--include-crds",
	}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.Command != config.CommandImport || cfg.SourceMode != "chart" {
		t.Fatalf("unexpected command/mode: %#v", cfg)
	}
	if cfg.Input != "/tmp/chart" || cfg.Namespace != "ns1" || cfg.ReleaseName != "rel1" {
		t.Fatalf("unexpected chart cfg fields: %#v", cfg)
	}
	if len(cfg.ValuesFiles) != 2 || len(cfg.SetValues) != 1 || len(cfg.SetStringValues) != 1 || len(cfg.SetFileValues) != 1 || len(cfg.SetJSONValues) != 1 {
		t.Fatalf("unexpected list flags: %#v", cfg)
	}
	if !cfg.VerifyEquivalence || !cfg.AnalyzeTemplates || !cfg.IncludeCRDs {
		t.Fatalf("expected verify/analyze/include-crds enabled: %#v", cfg)
	}
	if len(cfg.APIVersions) != 2 {
		t.Fatalf("expected 2 api versions, got %#v", cfg.APIVersions)
	}
}

func TestParse_ChartRejectsUnsupportedImportStrategy(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"chart", "--path", "/tmp/chart", "--import-strategy", "bad"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "unsupported --import-strategy") {
		t.Fatalf("expected import strategy error, got %v", err)
	}
}

func TestParse_ManifestsRejectsVerifyEquivalence(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"manifests", "--path", "/tmp/x.yaml", "--verify-equivalence"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "--verify-equivalence is supported only in chart mode") {
		t.Fatalf("expected verify-equivalence mode error, got %v", err)
	}
}

func TestParse_ComposeAcceptsHealthcheckLivenessFlag(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{"compose", "--path", "/tmp/compose.yaml", "--compose-healthcheck-to-liveness"}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.SourceMode != "compose" || !cfg.ComposeHealthcheckToLiveness {
		t.Fatalf("expected compose mode + liveness flag, got %#v", cfg)
	}
}

func TestParse_InspectDefaultsAndNoOpenBrowser(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{"inspect", "--path", "/tmp/chart", "--no-open-browser"}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.Command != config.CommandInspect || !cfg.InspectWeb {
		t.Fatalf("unexpected inspect cfg: %#v", cfg)
	}
	if cfg.InspectOpenBrowser {
		t.Fatalf("expected browser auto-open disabled")
	}
	if cfg.ImportStrategy != config.ImportStrategyHelpersExperimental {
		t.Fatalf("unexpected inspect default strategy: %q", cfg.ImportStrategy)
	}
}

func TestParse_ComposeInspectFlags(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{
		"compose-inspect",
		"--path", "/tmp/compose.yaml",
		"--format", "json",
		"--web",
		"--addr", "127.0.0.1:9999",
		"--env", "prod",
		"--compose-healthcheck-to-liveness",
		"--no-open-browser",
	}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.Command != config.CommandComposeInspect || cfg.Input != "/tmp/compose.yaml" {
		t.Fatalf("unexpected cfg: %#v", cfg)
	}
	if !cfg.InspectWeb || cfg.InspectAddr != "127.0.0.1:9999" {
		t.Fatalf("unexpected inspect web flags: %#v", cfg)
	}
	if cfg.InspectOpenBrowser {
		t.Fatalf("expected no-open-browser to disable auto-open")
	}
	if cfg.ComposeFormat != "json" || cfg.Env != "prod" || !cfg.ComposeHealthcheckToLiveness {
		t.Fatalf("unexpected compose-inspect flags: %#v", cfg)
	}
}

func TestParse_ComposeInspectRejectsBadFormat(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"compose-inspect", "--path", "/tmp/compose.yaml", "--format", "xml"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "unsupported --format") {
		t.Fatalf("expected format error, got %v", err)
	}
}

func TestParse_DyffFlagsAndPositionals(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{"dyff", "--from", "/tmp/a.yaml", "--to", "/tmp/b.yaml", "--output", "/tmp/diff.txt"}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse dyff flags error: %v", err)
	}
	if cfg.Command != config.CommandDyff || cfg.DyffFrom != "/tmp/a.yaml" || cfg.DyffTo != "/tmp/b.yaml" || cfg.Output != "/tmp/diff.txt" {
		t.Fatalf("unexpected dyff cfg: %#v", cfg)
	}

	cfg, err = Parse([]string{"dyff", "/tmp/a.yaml", "/tmp/b.yaml"}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse dyff positional error: %v", err)
	}
	if cfg.Command != config.CommandDyff || cfg.DyffFrom != "/tmp/a.yaml" || cfg.DyffTo != "/tmp/b.yaml" {
		t.Fatalf("unexpected dyff positional cfg: %#v", cfg)
	}
}

func TestParse_DyffRequiresTwoInputs(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"dyff", "/tmp/a.yaml"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "--from and --to are required") {
		t.Fatalf("expected dyff required args error, got %v", err)
	}
}

func TestParse_DyffAdvancedFlags(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{
		"dyff",
		"--from", "/tmp/a.yaml",
		"--to", "/tmp/b.yaml",
		"--respect-order",
		"--ignore-whitespace",
		"--id", "meta.id",
		"--id", "spec.name",
		"--quiet",
		"--fail-on-diff",
		"--color", "always",
	}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.DyffIgnoreOrder {
		t.Fatalf("expected --respect-order to disable ignore-order")
	}
	if !cfg.DyffIgnoreWhitespace || !cfg.DyffQuiet || !cfg.DyffFailOnDiff {
		t.Fatalf("expected dyff flags enabled, got %#v", cfg)
	}
	if cfg.DyffColor != "always" {
		t.Fatalf("expected color always, got %q", cfg.DyffColor)
	}
	if len(cfg.DyffAdditionalIdentifiers) != 2 || cfg.DyffAdditionalIdentifiers[0] != "meta.id" {
		t.Fatalf("unexpected dyff ids: %#v", cfg.DyffAdditionalIdentifiers)
	}
}

func TestParse_DyffColorValidationAndNoColor(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{"dyff", "--no-color", "/tmp/a.yaml", "/tmp/b.yaml"}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.DyffColor != "never" {
		t.Fatalf("expected --no-color to set never, got %q", cfg.DyffColor)
	}
	_, err = Parse([]string{"dyff", "--color", "bad", "/tmp/a.yaml", "/tmp/b.yaml"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "unsupported --color") {
		t.Fatalf("expected color validation error, got %v", err)
	}
}

func TestParse_DyffInterspersedFlagsAndExtraOptions(t *testing.T) {
	var out, errOut bytes.Buffer
	cfg, err := Parse([]string{
		"dyff",
		"/tmp/a.yaml",
		"--no-color",
		"/tmp/b.yaml",
		"--stats",
		"--format", "json",
		"--label-from", "old",
		"--label-to", "new",
	}, &out, &errOut)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if cfg.DyffFrom != "/tmp/a.yaml" || cfg.DyffTo != "/tmp/b.yaml" {
		t.Fatalf("unexpected positional resolution: %#v", cfg)
	}
	if cfg.DyffColor != "never" || !cfg.DyffStats || cfg.DyffFormat != "json" {
		t.Fatalf("unexpected dyff options: %#v", cfg)
	}
	if cfg.DyffLabelFrom != "old" || cfg.DyffLabelTo != "new" {
		t.Fatalf("unexpected labels: %#v", cfg)
	}
}

func TestParse_DyffRejectsDoubleStdin(t *testing.T) {
	var out, errOut bytes.Buffer
	_, err := Parse([]string{"dyff", "-", "-"}, &out, &errOut)
	if err == nil || !strings.Contains(err.Error(), "only one input may be '-'") {
		t.Fatalf("expected double-stdin error, got %v", err)
	}
}
