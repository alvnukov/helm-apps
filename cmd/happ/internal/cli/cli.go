package cli

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
)

type stringSliceFlag []string

func (s *stringSliceFlag) String() string { return strings.Join(*s, ",") }
func (s *stringSliceFlag) Set(v string) error {
	*s = append(*s, v)
	return nil
}

func Parse(args []string, stdout, stderr io.Writer) (config.Config, error) {
	if len(args) == 0 {
		printRootUsage(stderr)
		return config.Config{}, errors.New("subcommand is required (chart or manifests)")
	}

	switch args[0] {
	case "-h", "--help", "help":
		printRootUsage(stdout)
		return config.Config{}, flag.ErrHelp
	case "chart":
		return parseSubcommand(config.CommandImport, "chart", args[1:], stdout)
	case "manifests":
		return parseSubcommand(config.CommandImport, "manifests", args[1:], stdout)
	case "compose":
		return parseSubcommand(config.CommandImport, "compose", args[1:], stdout)
	case "inspect":
		return parseInspect(args[1:], stdout)
	case "compose-inspect":
		return parseComposeInspect(args[1:], stdout)
	case "dyff":
		return parseDyff(args[1:], stdout)
	default:
		printRootUsage(stderr)
		return config.Config{}, fmt.Errorf("unknown subcommand %q", args[0])
	}
}

func printRootUsage(w io.Writer) {
	fmt.Fprintln(w, "happ imports Helm chart render output or raw manifests into a helm-apps-based consumer chart")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Usage:")
	fmt.Fprintln(w, "  happ chart --path <chart-dir> [helm-render-flags] [import-flags]")
	fmt.Fprintln(w, "  happ manifests --path <yaml-file-or-dir> [import-flags]")
	fmt.Fprintln(w, "  happ compose --path <compose.yml|dir> [import-flags]")
	fmt.Fprintln(w, "  happ inspect --path <chart-dir> [helm-render-flags] --web")
	fmt.Fprintln(w, "  happ compose-inspect --path <compose.yml> [--format yaml|json]")
	fmt.Fprintln(w, "  happ dyff --from <a.yaml> --to <b.yaml>")
	fmt.Fprintln(w)
	fmt.Fprintln(w, "Examples:")
	fmt.Fprintln(w, "  happ chart --path ./chart --values values.prod.yaml --set image.tag=1.2.3 --out-chart-dir ./my-app-imported")
	fmt.Fprintln(w, "  happ manifests --path ./rendered --out-chart-dir ./my-app-imported")
	fmt.Fprintln(w, "  happ compose --path ./docker-compose.yml --out-chart-dir ./my-app-imported")
	fmt.Fprintln(w, "  happ inspect --path ./chart --values values.yaml --web")
	fmt.Fprintln(w, "  happ compose-inspect --path ./docker-compose.yml --format yaml")
	fmt.Fprintln(w, "  happ dyff ./before.yaml ./after.yaml")
	fmt.Fprintln(w, "  happ dyff - ./after.yaml --ignore-whitespace --fail-on-diff")
}

func printSubcommandUsage(mode string, w io.Writer) {
	printRootUsage(w)
	fmt.Fprintln(w)
	if mode == "chart" {
		fmt.Fprintln(w, "chart-specific flags:")
		fmt.Fprintln(w, "  --values, --set, --set-string, --set-file, --set-json, --namespace, --release-name")
		fmt.Fprintln(w, "  --kube-version, --api-version, --include-crds, --write-rendered-output")
	} else if mode == "manifests" {
		fmt.Fprintln(w, "manifests-specific flags:")
		fmt.Fprintln(w, "  --path <yaml-file-or-dir>")
	} else if mode == "compose" {
		fmt.Fprintln(w, "compose-specific flags:")
		fmt.Fprintln(w, "  --path <compose.yml|compose.yaml|docker-compose.yml|dir>")
		fmt.Fprintln(w, "  --compose-healthcheck-to-liveness (opt-in, duplicates healthcheck into livenessProbe)")
	}
	fmt.Fprintln(w)
	fmt.Fprintln(w, "common import flags:")
	fmt.Fprintln(w, "  --env, --group-name, --group-type, --min-include-bytes, --include-status")
	fmt.Fprintln(w, "  --output, --write-renderer-template, --out-chart-dir, --chart-name, --library-chart-path")
	fmt.Fprintln(w, "  --import-strategy raw|helpers-experimental")
	fmt.Fprintln(w, "  --verify-equivalence (chart mode)")
	fmt.Fprintln(w, "  --analyze-templates, --analyze-report (chart mode)")
}

func parseSubcommand(command, mode string, args []string, stdout io.Writer) (config.Config, error) {
	cfg := config.Config{
		Command:         command,
		SourceMode:      mode,
		Env:             "dev",
		GroupName:       config.DefaultGroupName,
		GroupType:       config.DefaultGroupType,
		ImportStrategy:  config.ImportStrategyRaw,
		MinIncludeBytes: 24,
		ReleaseName:     "imported",
		HelmExecTimeout: 2 * time.Minute,
		AnalyzeTimeout:  30 * time.Second,
		VerifyTimeout:   5 * time.Minute,
	}

	fs := flag.NewFlagSet("happ "+mode, flag.ContinueOnError)
	fs.SetOutput(io.Discard)

	var path string
	var inputAlias string
	var valuesFiles, setValues, setStringValues, setFileValues, setJSONValues, apiVersions stringSliceFlag

	fs.StringVar(&path, "path", "", "Path to chart dir (chart), YAML file/dir (manifests), or compose file/dir (compose)")
	fs.StringVar(&inputAlias, "input", "", "Alias for --path")

	if mode == "chart" {
		fs.StringVar(&cfg.ReleaseName, "release-name", cfg.ReleaseName, "Release name for helm template")
		fs.StringVar(&cfg.Namespace, "namespace", "", "Namespace for helm template")
		fs.Var(&valuesFiles, "values", "Values file for helm template (repeatable)")
		fs.Var(&setValues, "set", "Set value for helm template (repeatable)")
		fs.Var(&setStringValues, "set-string", "Set STRING value for helm template (repeatable)")
		fs.Var(&setFileValues, "set-file", "Set FILE value for helm template (repeatable)")
		fs.Var(&setJSONValues, "set-json", "Set JSON value for helm template (repeatable)")
		fs.StringVar(&cfg.KubeVersion, "kube-version", "", "Kubernetes version for helm template")
		fs.Var(&apiVersions, "api-version", "Kubernetes API version to register for helm template (repeatable)")
		fs.BoolVar(&cfg.IncludeCRDs, "include-crds", false, "Pass --include-crds to helm template")
		fs.DurationVar(&cfg.HelmExecTimeout, "helm-exec-timeout", cfg.HelmExecTimeout, "Timeout for helm template process (e.g. 30s, 2m)")
		fs.DurationVar(&cfg.VerifyTimeout, "verify-timeout", cfg.VerifyTimeout, "Timeout for equivalence verification stage (0 disables)")
		fs.StringVar(&cfg.RenderedOutput, "write-rendered-output", "", "Write intermediate helm template output to file")
	}

	fs.StringVar(&cfg.Env, "env", cfg.Env, "Set global.env in generated values")
	fs.StringVar(&cfg.GroupName, "group-name", cfg.GroupName, "Top-level custom group name")
	fs.StringVar(&cfg.GroupType, "group-type", cfg.GroupType, "Custom renderer type name")
	fs.IntVar(&cfg.MinIncludeBytes, "min-include-bytes", cfg.MinIncludeBytes, "Minimum string size for include extraction")
	fs.BoolVar(&cfg.IncludeStatus, "include-status", false, "Keep top-level status field")
	fs.StringVar(&cfg.Output, "output", "", "Write generated values YAML to file instead of stdout")
	fs.StringVar(&cfg.RendererOutput, "write-renderer-template", "", "Write custom renderer template (.tpl)")
	fs.StringVar(&cfg.OutChartDir, "out-chart-dir", "", "Generate full consumer chart in this directory")
	fs.StringVar(&cfg.ConsumerChartName, "chart-name", "", "Name for generated consumer chart (default: derived from input)")
	fs.StringVar(&cfg.LibraryChartPath, "library-chart-path", "", "Path to local helm-apps library chart to vendor into generated chart")
	fs.BoolVar(&cfg.VerifyEquivalence, "verify-equivalence", false, "Render source and generated chart and compare manifests (chart mode)")
	fs.StringVar(&cfg.ImportStrategy, "import-strategy", cfg.ImportStrategy, "Import strategy: raw or helpers-experimental")
	fs.BoolVar(&cfg.AnalyzeTemplates, "analyze-templates", false, "Analyze chart templates (.Values usage, pipes, risky constructs) and produce report (chart mode)")
	fs.StringVar(&cfg.AnalyzeReportPath, "analyze-report", "", "Write template analysis report JSON to file (default: stderr when --analyze-templates)")
	fs.DurationVar(&cfg.AnalyzeTimeout, "analyze-timeout", cfg.AnalyzeTimeout, "Timeout for template analysis stage (0 disables)")
	fs.DurationVar(&cfg.PipelineTimeout, "pipeline-timeout", 0, "Timeout for whole import/inspect pipeline (0 disables)")
	if mode == "compose" {
		fs.BoolVar(&cfg.ComposeHealthcheckToLiveness, "compose-healthcheck-to-liveness", false, "Also map compose healthcheck to livenessProbe (default: readinessProbe only)")
	}

	help := fs.Bool("help", false, "Show help")
	fs.BoolVar(help, "h", false, "Show help")

	if err := fs.Parse(args); err != nil {
		return cfg, err
	}
	if *help {
		printSubcommandUsage(mode, stdout)
		return cfg, flag.ErrHelp
	}

	if path == "" {
		path = inputAlias
	}
	if path == "" {
		return cfg, errors.New("--path is required")
	}
	cfg.Input = path
	cfg.ValuesFiles = valuesFiles
	cfg.SetValues = setValues
	cfg.SetStringValues = setStringValues
	cfg.SetFileValues = setFileValues
	cfg.SetJSONValues = setJSONValues
	cfg.APIVersions = apiVersions
	if cfg.VerifyEquivalence && mode != "chart" {
		return cfg, errors.New("--verify-equivalence is supported only in chart mode")
	}
	if cfg.AnalyzeTemplates && mode != "chart" {
		return cfg, errors.New("--analyze-templates is supported only in chart mode")
	}
	switch cfg.ImportStrategy {
	case config.ImportStrategyRaw, config.ImportStrategyHelpersExperimental:
	default:
		return cfg, fmt.Errorf("unsupported --import-strategy %q (expected %q or %q)", cfg.ImportStrategy, config.ImportStrategyRaw, config.ImportStrategyHelpersExperimental)
	}

	if len(fs.Args()) > 0 {
		return cfg, fmt.Errorf("unexpected positional args: %s", strings.Join(fs.Args(), " "))
	}
	return cfg, nil
}

func parseInspect(args []string, stdout io.Writer) (config.Config, error) {
	cfg := config.Config{
		Command:            config.CommandInspect,
		SourceMode:         "chart",
		ReleaseName:        "inspect",
		InspectAddr:        "127.0.0.1:8088",
		InspectWeb:         true,
		InspectOpenBrowser: true,
		Env:                "dev",
		GroupName:          config.DefaultGroupName,
		GroupType:          config.DefaultGroupType,
		ImportStrategy:     config.ImportStrategyHelpersExperimental,
		MinIncludeBytes:    24,
		HelmExecTimeout:    2 * time.Minute,
		AnalyzeTimeout:     30 * time.Second,
		VerifyTimeout:      5 * time.Minute,
	}
	fs := flag.NewFlagSet("happ inspect", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	var path string
	var inputAlias string
	var valuesFiles, setValues, setStringValues, setFileValues, setJSONValues, apiVersions stringSliceFlag
	fs.StringVar(&path, "path", "", "Path to chart dir")
	fs.StringVar(&inputAlias, "input", "", "Alias for --path")
	fs.StringVar(&cfg.ReleaseName, "release-name", cfg.ReleaseName, "Release name for helm template")
	fs.StringVar(&cfg.Namespace, "namespace", "", "Namespace for helm template")
	fs.Var(&valuesFiles, "values", "Values file for helm template (repeatable)")
	fs.Var(&setValues, "set", "Set value for helm template (repeatable)")
	fs.Var(&setStringValues, "set-string", "Set STRING value for helm template (repeatable)")
	fs.Var(&setFileValues, "set-file", "Set FILE value for helm template (repeatable)")
	fs.Var(&setJSONValues, "set-json", "Set JSON value for helm template (repeatable)")
	fs.StringVar(&cfg.KubeVersion, "kube-version", "", "Kubernetes version for helm template")
	fs.Var(&apiVersions, "api-version", "Kubernetes API version to register for helm template (repeatable)")
	fs.BoolVar(&cfg.IncludeCRDs, "include-crds", false, "Pass --include-crds to helm template")
	fs.DurationVar(&cfg.HelmExecTimeout, "helm-exec-timeout", cfg.HelmExecTimeout, "Timeout for helm template process (e.g. 30s, 2m)")
	fs.DurationVar(&cfg.AnalyzeTimeout, "analyze-timeout", cfg.AnalyzeTimeout, "Timeout for template analysis stage (0 disables)")
	fs.DurationVar(&cfg.PipelineTimeout, "pipeline-timeout", 0, "Timeout for whole inspect pipeline (0 disables, excludes time while web UI is serving)")
	fs.BoolVar(&cfg.InspectWeb, "web", true, "Run local web UI")
	fs.StringVar(&cfg.InspectAddr, "addr", cfg.InspectAddr, "Listen address for web UI (e.g. 127.0.0.1:8088)")
	fs.BoolVar(&cfg.InspectOpenBrowser, "open-browser", cfg.InspectOpenBrowser, "Open inspect UI in default browser automatically")
	noOpenBrowser := fs.Bool("no-open-browser", false, "Disable automatic browser opening")
	fs.StringVar(&cfg.Env, "env", cfg.Env, "Set global.env in generated preview values")
	fs.StringVar(&cfg.GroupName, "group-name", cfg.GroupName, "Top-level custom group name for preview")
	fs.StringVar(&cfg.GroupType, "group-type", cfg.GroupType, "Custom renderer type name for preview")
	fs.StringVar(&cfg.ImportStrategy, "import-strategy", cfg.ImportStrategy, "Import strategy for preview: raw or helpers-experimental")
	fs.BoolVar(&cfg.AnalyzeTemplates, "analyze-templates", true, "Analyze templates and include data in inspect UI/report")
	fs.StringVar(&cfg.AnalyzeReportPath, "analyze-report", "", "Write template analysis report JSON to file")
	help := fs.Bool("help", false, "Show help")
	fs.BoolVar(help, "h", false, "Show help")
	if err := fs.Parse(args); err != nil {
		return cfg, err
	}
	if *help {
		printRootUsage(stdout)
		return cfg, flag.ErrHelp
	}
	if path == "" {
		path = inputAlias
	}
	if path == "" {
		return cfg, errors.New("--path is required")
	}
	cfg.Input = path
	cfg.ValuesFiles = valuesFiles
	cfg.SetValues = setValues
	cfg.SetStringValues = setStringValues
	cfg.SetFileValues = setFileValues
	cfg.SetJSONValues = setJSONValues
	cfg.APIVersions = apiVersions
	if *noOpenBrowser {
		cfg.InspectOpenBrowser = false
	}
	switch cfg.ImportStrategy {
	case config.ImportStrategyRaw, config.ImportStrategyHelpersExperimental:
	default:
		return cfg, fmt.Errorf("unsupported --import-strategy %q (expected %q or %q)", cfg.ImportStrategy, config.ImportStrategyRaw, config.ImportStrategyHelpersExperimental)
	}
	if len(fs.Args()) > 0 {
		return cfg, fmt.Errorf("unexpected positional args: %s", strings.Join(fs.Args(), " "))
	}
	return cfg, nil
}

func parseComposeInspect(args []string, stdout io.Writer) (config.Config, error) {
	cfg := config.Config{
		Command:            config.CommandComposeInspect,
		ComposeFormat:      "yaml",
		InspectAddr:        "127.0.0.1:8089",
		InspectOpenBrowser: true,
	}
	fs := flag.NewFlagSet("happ compose-inspect", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	var path string
	var inputAlias string
	fs.StringVar(&path, "path", "", "Path to docker compose file or directory containing compose.yaml/docker-compose.yml")
	fs.StringVar(&inputAlias, "input", "", "Alias for --path")
	fs.StringVar(&cfg.ComposeFormat, "format", cfg.ComposeFormat, "Output format: yaml or json")
	fs.StringVar(&cfg.Output, "output", "", "Write report to file instead of stdout")
	fs.BoolVar(&cfg.InspectWeb, "web", false, "Run local web UI for compose graph and helm-apps preview")
	fs.StringVar(&cfg.InspectAddr, "addr", cfg.InspectAddr, "Listen address for web UI (e.g. 127.0.0.1:8089)")
	fs.BoolVar(&cfg.InspectOpenBrowser, "open-browser", cfg.InspectOpenBrowser, "Open compose inspect UI in default browser automatically")
	noOpenBrowser := fs.Bool("no-open-browser", false, "Disable automatic browser opening")
	fs.StringVar(&cfg.Env, "env", "dev", "Set global.env in generated helm-apps preview values")
	fs.BoolVar(&cfg.ComposeHealthcheckToLiveness, "compose-healthcheck-to-liveness", false, "Also map compose healthcheck to livenessProbe in helm-apps preview (default: readinessProbe only)")
	help := fs.Bool("help", false, "Show help")
	fs.BoolVar(help, "h", false, "Show help")
	if err := fs.Parse(args); err != nil {
		return cfg, err
	}
	if *help {
		printRootUsage(stdout)
		return cfg, flag.ErrHelp
	}
	if path == "" {
		path = inputAlias
	}
	if path == "" {
		return cfg, errors.New("--path is required")
	}
	cfg.Input = path
	if *noOpenBrowser {
		cfg.InspectOpenBrowser = false
	}
	switch strings.ToLower(strings.TrimSpace(cfg.ComposeFormat)) {
	case "yaml", "yml", "json":
	default:
		return cfg, fmt.Errorf("unsupported --format %q (expected yaml or json)", cfg.ComposeFormat)
	}
	if len(fs.Args()) > 0 {
		return cfg, fmt.Errorf("unexpected positional args: %s", strings.Join(fs.Args(), " "))
	}
	return cfg, nil
}

func parseDyff(args []string, stdout io.Writer) (config.Config, error) {
	cfg := config.Config{
		Command:         config.CommandDyff,
		DyffIgnoreOrder: true,
		DyffColor:       "auto",
		DyffFormat:      "text",
	}
	args = normalizeDyffArgs(args)
	fs := flag.NewFlagSet("happ dyff", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	fs.StringVar(&cfg.DyffFrom, "from", "", "Path to source YAML file")
	fs.StringVar(&cfg.DyffTo, "to", "", "Path to target YAML file")
	fs.StringVar(&cfg.Output, "output", "", "Write diff to file instead of stdout")
	fs.BoolVar(&cfg.DyffIgnoreOrder, "ignore-order", cfg.DyffIgnoreOrder, "Ignore list order changes when possible (default true)")
	respectOrder := fs.Bool("respect-order", false, "Disable order-insensitive list matching")
	fs.BoolVar(&cfg.DyffIgnoreWhitespace, "ignore-whitespace", false, "Ignore leading/trailing whitespace changes in strings")
	var ids stringSliceFlag
	fs.Var(&ids, "id", "Additional list-item identifier path for matching (repeatable, e.g. spec.name)")
	fs.BoolVar(&cfg.DyffQuiet, "quiet", false, "Do not print diff; use exit code only (0 equal, 1 different)")
	fs.BoolVar(&cfg.DyffFailOnDiff, "fail-on-diff", false, "Return non-zero exit code when differences are found")
	fs.StringVar(&cfg.DyffColor, "color", cfg.DyffColor, "Color mode: auto|always|never")
	fs.StringVar(&cfg.DyffFormat, "format", cfg.DyffFormat, "Output format: text or json")
	fs.BoolVar(&cfg.DyffStats, "stats", false, "Print summary stats (text mode only)")
	fs.StringVar(&cfg.DyffLabelFrom, "label-from", "", "Custom label for source side in text header")
	fs.StringVar(&cfg.DyffLabelTo, "label-to", "", "Custom label for target side in text header")
	noColor := fs.Bool("no-color", false, "Disable color output (same as --color=never)")
	help := fs.Bool("help", false, "Show help")
	fs.BoolVar(help, "h", false, "Show help")
	if err := fs.Parse(args); err != nil {
		return cfg, err
	}
	if *help {
		printRootUsage(stdout)
		return cfg, flag.ErrHelp
	}
	rest := fs.Args()
	if cfg.DyffFrom == "" && len(rest) > 0 {
		cfg.DyffFrom = rest[0]
		rest = rest[1:]
	}
	if cfg.DyffTo == "" && len(rest) > 0 {
		cfg.DyffTo = rest[0]
		rest = rest[1:]
	}
	if cfg.DyffFrom == "" || cfg.DyffTo == "" {
		return cfg, errors.New("--from and --to are required (or provide two positional args)")
	}
	if cfg.DyffFrom == "-" && cfg.DyffTo == "-" {
		return cfg, errors.New("only one input may be '-' (stdin)")
	}
	if *respectOrder {
		cfg.DyffIgnoreOrder = false
	}
	if *noColor {
		cfg.DyffColor = "never"
	}
	switch strings.ToLower(strings.TrimSpace(cfg.DyffColor)) {
	case "auto", "always", "never":
		cfg.DyffColor = strings.ToLower(strings.TrimSpace(cfg.DyffColor))
	default:
		return cfg, fmt.Errorf("unsupported --color %q (expected auto, always, or never)", cfg.DyffColor)
	}
	switch strings.ToLower(strings.TrimSpace(cfg.DyffFormat)) {
	case "text", "json":
		cfg.DyffFormat = strings.ToLower(strings.TrimSpace(cfg.DyffFormat))
	default:
		return cfg, fmt.Errorf("unsupported --format %q (expected text or json)", cfg.DyffFormat)
	}
	cfg.DyffAdditionalIdentifiers = ids
	if len(rest) > 0 {
		return cfg, fmt.Errorf("unexpected positional args: %s", strings.Join(rest, " "))
	}
	return cfg, nil
}

func normalizeDyffArgs(args []string) []string {
	if len(args) == 0 {
		return args
	}
	boolFlags := map[string]struct{}{
		"--ignore-order":      {},
		"--respect-order":     {},
		"--ignore-whitespace": {},
		"--quiet":             {},
		"--fail-on-diff":      {},
		"--stats":             {},
		"--no-color":          {},
		"--help":              {},
		"-h":                  {},
	}
	valueFlags := map[string]struct{}{
		"--from":       {},
		"--to":         {},
		"--output":     {},
		"--id":         {},
		"--color":      {},
		"--format":     {},
		"--label-from": {},
		"--label-to":   {},
	}
	flags := make([]string, 0, len(args))
	pos := make([]string, 0, 2)
	for i := 0; i < len(args); i++ {
		a := args[i]
		if a == "-" {
			pos = append(pos, a)
			continue
		}
		if strings.HasPrefix(a, "--") {
			if _, ok := boolFlags[a]; ok {
				flags = append(flags, a)
				continue
			}
			if _, ok := valueFlags[a]; ok {
				flags = append(flags, a)
				if i+1 < len(args) {
					i++
					flags = append(flags, args[i])
				}
				continue
			}
			if strings.Contains(a, "=") {
				flags = append(flags, a)
				continue
			}
			// unknown --token: keep as positional to preserve parser error shape later
			pos = append(pos, a)
			continue
		}
		if strings.HasPrefix(a, "-") && a != "-" {
			// keep short/unknown flags as flags so flag package reports properly.
			flags = append(flags, a)
			continue
		}
		pos = append(pos, a)
	}
	out := make([]string, 0, len(flags)+len(pos))
	out = append(out, flags...)
	out = append(out, pos...)
	return out
}
