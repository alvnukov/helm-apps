package app

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/analyze"
	"github.com/zol/helm-apps/cmd/happ/internal/composeimport"
	"github.com/zol/helm-apps/cmd/happ/internal/composeinspect"
	"github.com/zol/helm-apps/cmd/happ/internal/config"
	"github.com/zol/helm-apps/cmd/happ/internal/convert"
	"github.com/zol/helm-apps/cmd/happ/internal/dyffcli"
	"github.com/zol/helm-apps/cmd/happ/internal/dyfflike"
	"github.com/zol/helm-apps/cmd/happ/internal/inspectweb"
	"github.com/zol/helm-apps/cmd/happ/internal/output"
	"github.com/zol/helm-apps/cmd/happ/internal/source"
	"github.com/zol/helm-apps/cmd/happ/internal/verify"
	"gopkg.in/yaml.v3"
)

type Service struct{}

var errDyffDifferent = errors.New("dyff differences found")

func (Service) Run(cfg config.Config) error {
	if cfg.PipelineTimeout > 0 && cfg.Command != config.CommandInspect {
		return runWithTimeout(cfg.PipelineTimeout, "pipeline", func() error { return runPipeline(cfg) })
	}
	return runPipeline(cfg)
}

func runPipeline(cfg config.Config) error {
	if cfg.Command == config.CommandComposeInspect {
		rep, err := composeinspect.Load(cfg.Input)
		if err != nil {
			return err
		}
		if cfg.InspectWeb {
			sourceComposeYAML := ""
			if b, rerr := os.ReadFile(rep.SourcePath); rerr == nil {
				sourceComposeYAML = string(b)
			}
			values, warnings, err := composeimport.BuildValues(cfg, rep)
			previewYAML := ""
			previewErr := ""
			if err != nil {
				previewErr = err.Error()
			} else {
				for _, w := range warnings {
					_, _ = fmt.Fprintln(os.Stderr, "compose-import warning:", w)
				}
				if b, yerr := output.ValuesYAML(values); yerr != nil {
					previewErr = yerr.Error()
				} else {
					previewYAML = string(b)
				}
			}
			return inspectweb.ServeCompose(cfg.InspectAddr, cfg.InspectOpenBrowser, rep, sourceComposeYAML, previewYAML, previewErr)
		}
		return composeinspect.ResolveAndWrite(cfg.Input, cfg.ComposeFormat, cfg.Output)
	}
	if cfg.Command == config.CommandDyff {
		fromYAML, err := readDyffInput(cfg.DyffFrom)
		if err != nil {
			return fmt.Errorf("read --from: %w", err)
		}
		toYAML, err := readDyffInput(cfg.DyffTo)
		if err != nil {
			return fmt.Errorf("read --to: %w", err)
		}
		diff, err := dyfflike.BetweenYAMLWithOptions(fromYAML, toYAML, dyfflike.Options{
			IgnoreOrderChanges:     cfg.DyffIgnoreOrder,
			IgnoreWhitespaceChange: cfg.DyffIgnoreWhitespace,
			AdditionalIdentifiers:  cfg.DyffAdditionalIdentifiers,
		})
		if err != nil {
			return err
		}
		hasDiff := strings.TrimSpace(diff) != ""
		var outputText string
		switch cfg.DyffFormat {
		case "json":
			j, err := dyffcli.JSON(diff)
			if err != nil {
				return err
			}
			outputText = string(j)
		default:
			outputText = diff
		}
		if cfg.Output != "" {
			if err := os.WriteFile(cfg.Output, []byte(outputText), 0o644); err != nil {
				return err
			}
		}
		if !cfg.DyffQuiet {
			switch cfg.DyffFormat {
			case "json":
				if _, err = fmt.Fprint(os.Stdout, outputText); err != nil {
					return err
				}
				if !strings.HasSuffix(outputText, "\n") {
					_, _ = fmt.Fprintln(os.Stdout)
				}
			default:
				if hasDiff {
					fromLabel, toLabel := dyffcli.LabelsFromPaths(cfg.DyffFrom, cfg.DyffTo)
					if cfg.DyffLabelFrom != "" {
						fromLabel = cfg.DyffLabelFrom
					}
					if cfg.DyffLabelTo != "" {
						toLabel = cfg.DyffLabelTo
					}
					rendered := dyffcli.Format(diff, dyffcli.Options{
						ColorMode:  dyffcli.ColorMode(cfg.DyffColor),
						FromLabel:  fromLabel,
						ToLabel:    toLabel,
						ShowHeader: true,
					})
					if strings.TrimSpace(rendered) == "" {
						rendered = diff
					}
					if cfg.DyffStats {
						stats := dyffcli.ComputeStats(dyffcli.ParseEntries(diff))
						rendered = fmt.Sprintf("%s\n%s\n", rendered, formatDyffStats(stats))
					}
					_, err = fmt.Fprint(os.Stdout, rendered)
					if err != nil {
						return err
					}
					if !strings.HasSuffix(rendered, "\n") {
						_, _ = fmt.Fprintln(os.Stdout)
					}
				} else if cfg.Output == "" {
					if cfg.DyffStats {
						_, _ = fmt.Fprintln(os.Stdout, "No differences. (added=0 removed=0 changed=0)")
					} else {
						_, _ = fmt.Fprintln(os.Stdout, "No differences.")
					}
				}
			}
		}
		if cfg.DyffFailOnDiff && hasDiff {
			return errDyffDifferent
		}
		return nil
	}
	if cfg.Command == config.CommandImport && cfg.SourceMode == "compose" {
		rep, err := composeinspect.Load(cfg.Input)
		if err != nil {
			return err
		}
		values, warnings, err := composeimport.BuildValues(cfg, rep)
		if err != nil {
			return err
		}
		for _, w := range warnings {
			_, _ = fmt.Fprintln(os.Stderr, "compose-import warning:", w)
		}
		libraryPath := cfg.LibraryChartPath
		if cfg.OutChartDir != "" {
			if libraryPath == "" {
				libraryPath = detectDefaultLibraryPath()
			}
			if err := output.GenerateConsumerChart(cfg.OutChartDir, cfg.ConsumerChartName, cfg.GroupType, values, libraryPath); err != nil {
				return err
			}
		}
		if cfg.OutChartDir == "" || cfg.Output != "" {
			if err := output.WriteValues(cfg.Output, values); err != nil {
				return err
			}
		}
		return nil
	}

	var analysisRep *analyze.Report
	if cfg.AnalyzeTemplates {
		var rep analyze.Report
		err := runWithTimeout(cfg.AnalyzeTimeout, "analyze templates", func() error {
			var err error
			rep, err = analyze.Chart(cfg.Input)
			return err
		})
		if err != nil {
			return fmt.Errorf("analyze templates: %w", err)
		}
		analysisRep = &rep
		shouldWriteAnalysisReport := cfg.AnalyzeReportPath != ""
		if cfg.Command != config.CommandInspect && cfg.AnalyzeReportPath == "" {
			shouldWriteAnalysisReport = true
		}
		if shouldWriteAnalysisReport {
			if err := analyze.WriteReport(cfg.AnalyzeReportPath, rep); err != nil {
				return fmt.Errorf("write analysis report: %w", err)
			}
		}
	}

	docs, err := source.LoadDocuments(cfg)
	if err != nil {
		return err
	}
	if cfg.Command == config.CommandInspect {
		model := buildInspectModel(cfg, docs, analysisRep)
		if !cfg.InspectWeb {
			return fmt.Errorf("inspect mode currently supports only --web")
		}
		hooks := &inspectweb.InteractiveHooks{
			Enabled:           cfg.SourceMode == "chart",
			SourceValuesYAML:  loadChartValuesYAML(cfg.Input),
			SuggestedSavePath: filepath.Join(cfg.Input, "values.inspect.yaml"),
			RunExperiment: func(valuesYAML string) (inspectweb.ExperimentResponse, error) {
				tmp, err := os.CreateTemp("", "happ-values-*.yaml")
				if err != nil {
					return inspectweb.ExperimentResponse{}, err
				}
				tmpPath := tmp.Name()
				_ = tmp.Close()
				defer os.Remove(tmpPath)
				if err := os.WriteFile(tmpPath, []byte(valuesYAML), 0o644); err != nil {
					return inspectweb.ExperimentResponse{}, err
				}
				expCfg := cfg
				expCfg.ValuesFiles = append([]string{tmpPath}, cfg.ValuesFiles...)
				expDocs, err := source.LoadDocuments(expCfg)
				if err != nil {
					return inspectweb.ExperimentResponse{}, err
				}
				expModel := buildInspectModel(expCfg, expDocs, analysisRep)
				return inspectweb.ExperimentResponse{
					Model: expModel,
					Diff:  inspectweb.DiffModels(model, expModel),
				}, nil
			},
			CompareEntities: func(valuesYAML string) (inspectweb.CompareEntitiesResponse, error) {
				tmpValues, err := os.CreateTemp("", "happ-values-*.yaml")
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}
				tmpValuesPath := tmpValues.Name()
				_ = tmpValues.Close()
				defer os.Remove(tmpValuesPath)
				if err := os.WriteFile(tmpValuesPath, []byte(valuesYAML), 0o644); err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}

				expCfg := cfg
				expCfg.ValuesFiles = append([]string{tmpValuesPath}, cfg.ValuesFiles...)
				if expCfg.Namespace == "" {
					expCfg.Namespace = "default"
				}
				sourceDocs, err := source.LoadDocuments(expCfg)
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}
				importedValues, err := convert.BuildValues(expCfg, sourceDocs)
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}

				tmpChartDir, err := os.MkdirTemp("", "happ-compare-chart-*")
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}
				defer os.RemoveAll(tmpChartDir)
				libraryPath := cfg.LibraryChartPath
				if libraryPath == "" {
					libraryPath = detectDefaultLibraryPath()
				}
				if err := output.GenerateConsumerChart(tmpChartDir, "happ-compare", cfg.GroupType, importedValues, libraryPath); err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}

				genCfg := expCfg
				genCfg.Input = tmpChartDir
				genCfg.ValuesFiles = nil
				genCfg.SetValues = nil
				genCfg.SetStringValues = nil
				genCfg.SetFileValues = nil
				genCfg.SetJSONValues = nil
				genCfg.RenderedOutput = ""
				genRendered, err := source.RenderChartBytes(genCfg, tmpChartDir)
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, fmt.Errorf("render generated chart: %w", err)
				}
				genDocs, err := source.ParseDocuments(genRendered)
				if err != nil {
					return inspectweb.CompareEntitiesResponse{}, err
				}
				srcNorm := verify.NormalizeDocsForCompare(sourceDocs)
				genNorm := verify.NormalizeDocsForCompare(genDocs)
				detailed := verify.CompareDetailed(sourceDocs, genDocs)
				dyffByKey := map[string]string{}
				srcIdx := docsByKey(srcNorm)
				genIdx := docsByKey(genNorm)
				if verify.DyffAvailable() {
					for _, r := range detailed.Resources {
						if r.Status != "changed" {
							continue
						}
						sd, sok := srcIdx[r.Key]
						gd, gok := genIdx[r.Key]
						if !sok || !gok {
							continue
						}
						if diff, err := verify.DyffBetweenDocs(sd, gd); err == nil && strings.TrimSpace(diff) != "" {
							dyffByKey[r.Key] = diff
						}
					}
				}
				return inspectweb.CompareEntitiesResponse{
					Compare:            detailed,
					SourceYAMLByKey:    docsYAMLByKey(srcNorm),
					GeneratedYAMLByKey: docsYAMLByKey(genNorm),
					DyffByKey:          dyffByKey,
					DyffAvailable:      verify.DyffAvailable(),
				}, nil
			},
			SaveValues: func(path, valuesYAML string) error {
				if path == "" {
					return fmt.Errorf("save path is required")
				}
				return os.WriteFile(path, []byte(valuesYAML), 0o644)
			},
		}
		// pipeline-timeout excludes long-lived web serving; apply only to pre-serve stages.
		return inspectweb.Serve(cfg.InspectAddr, cfg.InspectOpenBrowser, model, hooks)
	}
	values, err := convert.BuildValues(cfg, docs)
	if err != nil {
		return err
	}

	libraryPath := cfg.LibraryChartPath
	if cfg.OutChartDir != "" {
		if libraryPath == "" {
			libraryPath = detectDefaultLibraryPath()
		}
		if err := output.GenerateConsumerChart(cfg.OutChartDir, cfg.ConsumerChartName, cfg.GroupType, values, libraryPath); err != nil {
			return err
		}
	}

	if cfg.OutChartDir == "" || cfg.Output != "" {
		if err := output.WriteValues(cfg.Output, values); err != nil {
			return err
		}
	}
	if cfg.OutChartDir == "" && cfg.RendererOutput != "" {
		if err := output.WriteRendererTemplate(cfg.RendererOutput, cfg.GroupType); err != nil {
			return err
		}
	}

	if cfg.VerifyEquivalence {
		if cfg.SourceMode != "chart" {
			return fmt.Errorf("--verify-equivalence requires chart mode")
		}
		if cfg.OutChartDir == "" {
			return fmt.Errorf("--verify-equivalence requires --out-chart-dir")
		}
		genCfg := cfg
		genCfg.Input = cfg.OutChartDir
		genCfg.ValuesFiles = nil
		genCfg.SetValues = nil
		genCfg.SetStringValues = nil
		genCfg.SetFileValues = nil
		genCfg.SetJSONValues = nil
		genCfg.RenderedOutput = ""
		if err := runWithTimeout(cfg.VerifyTimeout, "verify equivalence", func() error {
			genRendered, err := source.RenderChartBytes(genCfg, cfg.OutChartDir)
			if err != nil {
				return fmt.Errorf("render generated chart: %w", err)
			}
			genDocs, err := source.ParseDocuments(genRendered)
			if err != nil {
				return err
			}
			res := verify.Equivalent(docs, genDocs)
			if !res.Equal {
				return fmt.Errorf("equivalence check failed: %s", res.Summary)
			}
			_, _ = fmt.Fprintln(os.Stderr, "Equivalence check:", res.Summary)
			return nil
		}); err != nil {
			return err
		}
	}
	return nil
}

func formatDyffStats(s dyffcli.Stats) string {
	return fmt.Sprintf("stats: added=%d removed=%d changed=%d total=%d", s.Added, s.Removed, s.Changed, s.Total)
}

func readDyffInput(path string) ([]byte, error) {
	if path == "-" {
		return io.ReadAll(os.Stdin)
	}
	return os.ReadFile(path)
}

func docsByKey(docs []map[string]any) map[string]map[string]any {
	out := make(map[string]map[string]any, len(docs))
	for _, d := range docs {
		k := compareDocKey(d)
		out[k] = d
	}
	return out
}

func buildInspectModel(cfg config.Config, docs []map[string]any, analysisRep *analyze.Report) inspectweb.Model {
	var preview *inspectweb.HelmAppsPreview
	if values, convErr := convert.BuildValues(cfg, docs); convErr != nil {
		preview = &inspectweb.HelmAppsPreview{
			Strategy:  cfg.ImportStrategy,
			GroupName: cfg.GroupName,
			GroupType: cfg.GroupType,
			Error:     convErr.Error(),
		}
	} else if b, yErr := output.ValuesYAML(values); yErr != nil {
		preview = &inspectweb.HelmAppsPreview{
			Strategy:  cfg.ImportStrategy,
			GroupName: cfg.GroupName,
			GroupType: cfg.GroupType,
			Error:     yErr.Error(),
		}
	} else {
		preview = &inspectweb.HelmAppsPreview{
			Strategy:   cfg.ImportStrategy,
			GroupName:  cfg.GroupName,
			GroupType:  cfg.GroupType,
			ValuesYAML: string(b),
		}
	}
	return inspectweb.BuildModel(docs, analysisRep, preview)
}

func loadChartValuesYAML(chartPath string) string {
	b, err := os.ReadFile(filepath.Join(chartPath, "values.yaml"))
	if err != nil {
		return ""
	}
	return string(b)
}

func runWithTimeout(timeout time.Duration, stage string, fn func() error) error {
	if timeout <= 0 {
		return fn()
	}
	done := make(chan error, 1)
	go func() { done <- fn() }()
	select {
	case err := <-done:
		return err
	case <-time.After(timeout):
		return fmt.Errorf("%s timed out after %s", stage, timeout)
	}
}

func detectDefaultLibraryPath() string {
	candidates := []string{
		filepath.Join("charts", "helm-apps"),
		filepath.Join("..", "..", "charts", "helm-apps"),
	}
	for _, p := range candidates {
		if st, err := os.Stat(p); err == nil && st.IsDir() {
			return p
		}
	}
	return ""
}

func docsYAMLByKey(docs []map[string]any) map[string]string {
	out := make(map[string]string, len(docs))
	for _, d := range docs {
		key := compareDocKey(d)
		if key == "" {
			continue
		}
		b, err := yaml.Marshal(d)
		if err != nil {
			continue
		}
		out[key] = strings.TrimRight(string(b), "\n")
	}
	return out
}

func compareDocKey(d map[string]any) string {
	apiVersion, _ := d["apiVersion"].(string)
	kind, _ := d["kind"].(string)
	meta, _ := d["metadata"].(map[string]any)
	if meta == nil {
		return ""
	}
	name, _ := meta["name"].(string)
	ns, _ := meta["namespace"].(string)
	if ns == "" {
		ns = "default"
	}
	return strings.Join([]string{apiVersion, kind, ns, name}, "/")
}
