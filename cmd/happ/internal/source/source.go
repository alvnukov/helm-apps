package source

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
	"gopkg.in/yaml.v3"
)

const defaultHelmExecTimeout = 2 * time.Minute

type helmCommandRunner func(ctx context.Context, bin string, args []string) (stdout []byte, stderr []byte, err error)

var runHelmCommand helmCommandRunner = defaultHelmCommandRunner

func LoadDocuments(cfg config.Config) ([]map[string]any, error) {
	var raw []any
	var err error
	if cfg.SourceMode == "chart" {
		raw, err = loadChartDocs(cfg)
	} else {
		raw, err = loadManifestDocs(cfg.Input)
	}
	if err != nil {
		return nil, err
	}
	return flattenK8sLists(raw), nil
}

func RenderChartBytes(cfg config.Config, chartPath string) ([]byte, error) {
	return renderChartBytes(cfg, chartPath, false)
}

func ParseDocuments(data []byte) ([]map[string]any, error) {
	raw, err := parseYAMLStream(data)
	if err != nil {
		return nil, err
	}
	return flattenK8sLists(raw), nil
}

func loadChartDocs(cfg config.Config) ([]any, error) {
	b, err := renderChartBytes(cfg, cfg.Input, true)
	if err != nil {
		return nil, err
	}
	return parseYAMLStream(b)
}

func renderChartBytes(cfg config.Config, chartPath string, writeRenderedOutput bool) ([]byte, error) {
	args := []string{"template", cfg.ReleaseName, cfg.Input}
	args[2] = chartPath
	if cfg.Namespace != "" {
		args = append(args, "--namespace", cfg.Namespace)
	}
	for _, vf := range cfg.ValuesFiles {
		args = append(args, "--values", vf)
	}
	for _, v := range cfg.SetValues {
		args = append(args, "--set", v)
	}
	for _, v := range cfg.SetStringValues {
		args = append(args, "--set-string", v)
	}
	for _, v := range cfg.SetFileValues {
		args = append(args, "--set-file", v)
	}
	for _, v := range cfg.SetJSONValues {
		args = append(args, "--set-json", v)
	}
	if cfg.KubeVersion != "" {
		args = append(args, "--kube-version", cfg.KubeVersion)
	}
	for _, v := range cfg.APIVersions {
		args = append(args, "--api-versions", v)
	}
	if cfg.IncludeCRDs {
		args = append(args, "--include-crds")
	}

	timeout := cfg.HelmExecTimeout
	if timeout <= 0 {
		timeout = defaultHelmExecTimeout
	}
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()
	stdout, stderr, err := runHelmCommand(ctx, "helm", args)
	if err != nil {
		if errors.Is(ctx.Err(), context.DeadlineExceeded) {
			return nil, fmt.Errorf("helm template timed out after %s", timeout)
		}
		if errors.Is(err, exec.ErrNotFound) {
			return nil, errors.New("helm binary not found; install helm or use manifests mode")
		}
		msg := strings.TrimSpace(string(stderr))
		if msg == "" {
			msg = err.Error()
		}
		return nil, fmt.Errorf("helm template failed: %s", msg)
	}
	if writeRenderedOutput && cfg.RenderedOutput != "" {
		if err := os.WriteFile(cfg.RenderedOutput, stdout, 0o644); err != nil {
			return nil, err
		}
	}
	return stdout, nil
}

func loadManifestDocs(path string) ([]any, error) {
	files, err := collectManifestFiles(path)
	if err != nil {
		return nil, err
	}
	if len(files) == 0 {
		return nil, fmt.Errorf("no YAML files found at %s", path)
	}

	var docs []any
	for _, p := range files {
		b, err := os.ReadFile(p)
		if err != nil {
			return nil, err
		}
		part, err := parseYAMLStream(b)
		if err != nil {
			return nil, fmt.Errorf("invalid YAML in %s: %w", p, err)
		}
		docs = append(docs, part...)
	}
	return docs, nil
}

func collectManifestFiles(path string) ([]string, error) {
	st, err := os.Stat(path)
	if err != nil {
		return nil, err
	}
	if !st.IsDir() {
		return []string{path}, nil
	}

	var files []string
	err = filepath.Walk(path, func(p string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if info.IsDir() {
			return nil
		}
		low := strings.ToLower(info.Name())
		if strings.HasSuffix(low, ".yaml") || strings.HasSuffix(low, ".yml") {
			files = append(files, p)
		}
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Strings(files)
	return files, nil
}

func flattenK8sLists(docs []any) []map[string]any {
	out := make([]map[string]any, 0, len(docs))
	for _, d := range docs {
		m, ok := d.(map[string]any)
		if !ok {
			continue
		}
		if kind, _ := m["kind"].(string); kind == "List" {
			if items, ok := m["items"].([]any); ok {
				for _, item := range items {
					if im, ok := item.(map[string]any); ok {
						out = append(out, im)
					}
				}
			}
			continue
		}
		out = append(out, m)
	}
	return out
}

func parseYAMLStream(data []byte) ([]any, error) {
	dec := yaml.NewDecoder(bytes.NewReader(data))
	var docs []any
	for {
		var v any
		err := dec.Decode(&v)
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return nil, err
		}
		if v == nil {
			continue
		}
		docs = append(docs, normalizeYAML(v))
	}
	return docs, nil
}

func normalizeYAML(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[k] = normalizeYAML(vv)
		}
		return out
	case map[any]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[fmt.Sprint(k)] = normalizeYAML(vv)
		}
		return out
	case []any:
		for i := range x {
			x[i] = normalizeYAML(x[i])
		}
		return x
	default:
		return x
	}
}

func defaultHelmCommandRunner(ctx context.Context, bin string, args []string) ([]byte, []byte, error) {
	cmd := exec.CommandContext(ctx, bin, args...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	return stdout.Bytes(), stderr.Bytes(), err
}
