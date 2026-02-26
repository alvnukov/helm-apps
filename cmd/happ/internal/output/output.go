package output

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/zol/helm-apps/cmd/happ/assets"
	"gopkg.in/yaml.v3"
)

type literalString string

func (s literalString) MarshalYAML() (any, error) {
	return &yaml.Node{Kind: yaml.ScalarNode, Tag: "!!str", Value: string(s), Style: yaml.LiteralStyle}, nil
}

func WriteValues(path string, values map[string]any) error {
	b, err := ValuesYAML(values)
	if err != nil {
		return err
	}
	if path == "" {
		_, err = os.Stdout.Write(b)
		return err
	}
	return os.WriteFile(path, b, 0o644)
}

func ValuesYAML(values map[string]any) ([]byte, error) {
	b, err := yaml.Marshal(deepLiteralize(values))
	if err != nil {
		return nil, err
	}
	b, err = reorderTopLevelGlobalFirstYAML(b)
	if err != nil {
		return nil, err
	}
	return bytes.TrimPrefix(b, []byte("---\n")), nil
}

func WriteRendererTemplate(path, groupType string) error {
	if path == "" {
		return nil
	}
	if groupType == "" || groupType == "apps-k8s-manifests" {
		return nil
	}
	return os.WriteFile(path, []byte(RendererTemplate(groupType)), 0o644)
}

func GenerateConsumerChart(outDir, chartName, groupType string, values map[string]any, libraryChartPath string) error {
	if outDir == "" {
		return nil
	}
	if chartName == "" {
		chartName = filepath.Base(outDir)
	}
	if chartName == "." || chartName == string(filepath.Separator) || chartName == "" {
		chartName = "happ-imported"
	}

	if err := os.MkdirAll(filepath.Join(outDir, "templates"), 0o755); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Join(outDir, "charts"), 0o755); err != nil {
		return err
	}

	chartYAML := fmt.Sprintf("apiVersion: v2\nname: %s\nversion: 0.1.0\ntype: application\n", chartName)
	if err := os.WriteFile(filepath.Join(outDir, "Chart.yaml"), []byte(chartYAML), 0o644); err != nil {
		return err
	}
	if err := WriteValues(filepath.Join(outDir, "values.yaml"), values); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(outDir, "templates", "init-helm-apps-library.yaml"), []byte("{{- include \"apps-utils.init-library\" $ }}\n"), 0o644); err != nil {
		return err
	}
	if needsCustomRenderer(values, groupType) {
		if err := os.WriteFile(filepath.Join(outDir, "templates", "zz-imported-renderer.tpl"), []byte(RendererTemplate(groupType)), 0o644); err != nil {
			return err
		}
	}

	dst := filepath.Join(outDir, "charts", "helm-apps")
	if libraryChartPath != "" {
		if err := copyDir(libraryChartPath, dst); err != nil {
			return fmt.Errorf("copy helm-apps library chart: %w", err)
		}
	} else if assets.HasHelmAppsChart() {
		if err := assets.ExtractHelmAppsChart(dst); err != nil {
			return fmt.Errorf("extract embedded helm-apps library chart: %w", err)
		}
	}
	return nil
}

func needsCustomRenderer(values map[string]any, groupType string) bool {
	if groupType == "" || groupType == "apps-k8s-manifests" {
		return false
	}
	for k, v := range values {
		if k == "global" {
			continue
		}
		group, ok := v.(map[string]any)
		if !ok {
			continue
		}
		gv, ok := group["__GroupVars__"].(map[string]any)
		if !ok {
			continue
		}
		t, _ := gv["type"].(string)
		if t == groupType {
			return true
		}
	}
	return false
}

func RendererTemplate(groupType string) string {
	return strings.ReplaceAll(`{{- define "__GROUP_TYPE__.render" -}}
{{- $ := . -}}
{{- $app := $.CurrentApp -}}
{{- include "apps-utils.printPath" $ }}
apiVersion: {{ $app.apiVersion | quote }}
kind: {{ $app.kind | quote }}
metadata:
  name: {{ $app.metadataName | quote }}
{{- with $app.metadataNamespace }}
  namespace: {{ . | quote }}
{{- end }}
{{- with $app.metadataLabelsYAML }}
  labels:
{{ . | nindent 4 }}
{{- end }}
{{- with $app.metadataAnnotationsYAML }}
  annotations:
{{ . | nindent 4 }}
{{- end }}
{{- with $app.metadataRestYAML }}
{{ . | nindent 2 }}
{{- end }}
{{- if $app.fieldsScalars }}
{{- range $k := keys $app.fieldsScalars | sortAlpha }}
{{- $v := index $app.fieldsScalars $k }}
{{- $scalarYaml := (toYaml $v | trimSuffix "\n") }}
{{ $k }}: {{ $scalarYaml }}
{{- end }}
{{- end }}
{{- if $app.fieldsYAML }}
{{- range $k := keys $app.fieldsYAML | sortAlpha }}
{{ $k }}:
{{ index $app.fieldsYAML $k | nindent 2 }}
{{- end }}
{{- end }}
{{- end -}}
`, "__GROUP_TYPE__", groupType)
}

func deepLiteralize(v any) any {
	switch x := v.(type) {
	case string:
		if strings.Contains(x, "\n") {
			return literalString(x)
		}
		return x
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[k] = deepLiteralize(vv)
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = deepLiteralize(x[i])
		}
		return out
	default:
		return v
	}
}

func reorderTopLevelGlobalFirstYAML(in []byte) ([]byte, error) {
	var doc yaml.Node
	if err := yaml.Unmarshal(in, &doc); err != nil {
		return nil, err
	}
	if len(doc.Content) == 0 {
		return in, nil
	}
	root := doc.Content[0]
	if root.Kind != yaml.MappingNode || len(root.Content) < 4 {
		return in, nil
	}
	globalIdx := -1
	for i := 0; i+1 < len(root.Content); i += 2 {
		if root.Content[i].Value == "global" {
			globalIdx = i
			break
		}
	}
	if globalIdx <= 0 {
		return in, nil
	}
	key := root.Content[globalIdx]
	val := root.Content[globalIdx+1]
	rest := append([]*yaml.Node{}, root.Content[:globalIdx]...)
	rest = append(rest, root.Content[globalIdx+2:]...)
	root.Content = append([]*yaml.Node{key, val}, rest...)
	return yaml.Marshal(&doc)
}

func copyDir(src, dst string) error {
	if err := os.RemoveAll(dst); err != nil && !os.IsNotExist(err) {
		return err
	}
	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dst, rel)
		if info.IsDir() {
			return os.MkdirAll(target, info.Mode().Perm())
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return os.WriteFile(target, data, info.Mode().Perm())
	})
}
