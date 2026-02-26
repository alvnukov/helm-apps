package convert

import (
	"bytes"
	"encoding/json"
	"fmt"
	"hash/fnv"
	"sort"
	"strconv"
	"strings"

	"github.com/zol/helm-apps/cmd/happ/internal/config"
	"gopkg.in/yaml.v3"
)

type appEntry struct {
	Key string
	App map[string]any
}

type includeDescriptor struct {
	Kind      string
	Field     string
	Canonical string
	Apps      []string
}

var ignoredImportedMetadataLabelKeys = map[string]struct{}{
	"helm.sh/chart":                {},
	"app.kubernetes.io/managed-by": {},
	"app.kubernetes.io/instance":   {},
	"app.kubernetes.io/name":       {},
	"app.kubernetes.io/version":    {},
	"app.kubernetes.io/part-of":    {},
	"app.kubernetes.io/component":  {},
}

func BuildValues(cfg config.Config, docs []map[string]any) (map[string]any, error) {
	if cfg.ImportStrategy == config.ImportStrategyHelpersExperimental {
		return BuildValuesHelpersExperimental(cfg, docs)
	}

	entries := make([]appEntry, 0, len(docs))
	for i, doc := range docs {
		app, key, ok := convertDocument(cfg, doc, i)
		if !ok {
			continue
		}
		entries = append(entries, appEntry{Key: key, App: app})
	}
	if len(entries) == 0 {
		return nil, fmt.Errorf("no supported Kubernetes resources found in input")
	}

	group := map[string]any{}
	used := map[string]int{}
	for _, e := range entries {
		key := e.Key
		used[key]++
		if used[key] > 1 {
			key = fmt.Sprintf("%s-%s", key, shortStableHash(e.App))
		}
		group[key] = e.App
	}

	global := map[string]any{"env": cfg.Env}
	if includes := buildIncludes(cfg, group); len(includes) > 0 {
		global["_includes"] = includes
	}

	return map[string]any{
		"global":                global,
		config.DefaultGroupName: group,
	}, nil
}

func convertDocument(cfg config.Config, doc map[string]any, index int) (map[string]any, string, bool) {
	kind, _ := doc["kind"].(string)
	apiVersion, _ := doc["apiVersion"].(string)
	if kind == "" || apiVersion == "" {
		return nil, "", false
	}

	metadata := mapStringAny(getMap(doc, "metadata"))
	name, _ := metadata["name"].(string)
	ns, _ := metadata["namespace"].(string)
	if strings.TrimSpace(name) == "" {
		name = strings.ToLower(kind) + "-" + strconv.Itoa(index+1)
	}

	top := cloneMap(doc)
	delete(top, "apiVersion")
	delete(top, "kind")
	if !cfg.IncludeStatus {
		delete(top, "status")
	}

	app := map[string]any{
		"enabled":    true,
		"apiVersion": apiVersion,
		"kind":       kind,
	}
	if strings.TrimSpace(name) != "" {
		app["name"] = name
	}

	metaResidual := cloneMap(metadata)
	delete(metaResidual, "name")
	if labels, ok := metaResidual["labels"]; ok {
		filtered := filterImportedMetadataLabels(labels)
		if isBlankContainer(filtered) {
			delete(metaResidual, "labels")
		} else {
			metaResidual["labels"] = filtered
		}
	}
	if s := yamlBodySorted(cleanAny(metaResidual)); s != nil {
		app["metadata"] = *s
	}

	for _, k := range []string{"spec", "data", "stringData", "binaryData"} {
		if v, ok := top[k]; ok {
			if s := yamlBodySorted(cleanAny(v)); s != nil {
				app[k] = *s
			}
			delete(top, k)
		}
	}
	for _, k := range []string{"type", "immutable"} {
		if v, ok := top[k]; ok && v != nil {
			app[k] = v
			delete(top, k)
		}
	}
	delete(top, "metadata")
	if s := yamlBodySorted(cleanAny(top)); s != nil {
		app["extraFields"] = *s
	}

	return app, genericAppKey(kind, ns, name, apiVersion), true
}

func buildIncludes(cfg config.Config, group map[string]any) map[string]any {
	candidates := map[string]*includeDescriptor{}

	for _, appName := range sortedAppNames(group) {
		app, ok := group[appName].(map[string]any)
		if !ok {
			continue
		}
		for _, field := range []string{"metadata", "spec", "data", "stringData", "binaryData", "extraFields"} {
			if s, ok := app[field].(string); ok && len(s) >= cfg.MinIncludeBytes {
				key := "app_field|" + field + "|" + s
				cd := candidates[key]
				if cd == nil {
					cd = &includeDescriptor{Kind: "app_field", Field: field, Canonical: s}
					candidates[key] = cd
				}
				cd.Apps = append(cd.Apps, appName)
			}
		}
		scalars := map[string]any{}
		for _, scalarKey := range []string{"type", "immutable"} {
			if v, ok := app[scalarKey]; ok {
				scalars[scalarKey] = v
			}
		}
		if len(scalars) >= 2 {
			canon, err := canonicalJSON(scalars)
			if err == nil {
				key := "app_scalars_hash|" + canon
				cd := candidates[key]
				if cd == nil {
					cd = &includeDescriptor{Kind: "app_scalars_hash", Canonical: canon}
					candidates[key] = cd
				}
				cd.Apps = append(cd.Apps, appName)
			}
		}
	}

	keys := make([]string, 0, len(candidates))
	for k := range candidates {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	includes := map[string]any{}
	seq := 0
	for _, key := range keys {
		desc := candidates[key]
		if len(desc.Apps) < 2 {
			continue
		}
		body, err := includeBody(*desc)
		if err != nil {
			continue
		}
		seq++
		name := includeName(*desc, seq)
		includes[name] = body
		applyIncludeToApps(group, uniqStrings(desc.Apps), name, *desc)
	}
	return includes
}

func includeBody(desc includeDescriptor) (map[string]any, error) {
	switch desc.Kind {
	case "app_field":
		return map[string]any{desc.Field: desc.Canonical}, nil
	case "app_scalars_hash":
		var m map[string]any
		if err := json.Unmarshal([]byte(desc.Canonical), &m); err != nil {
			return nil, err
		}
		return m, nil
	default:
		return nil, fmt.Errorf("unknown include kind: %s", desc.Kind)
	}
}

func applyIncludeToApps(group map[string]any, appNames []string, includeName string, desc includeDescriptor) {
	for _, appName := range appNames {
		app, ok := group[appName].(map[string]any)
		if !ok {
			continue
		}
		includes, _ := app["_include"].([]any)
		if !containsStringAny(includes, includeName) {
			includes = append(includes, includeName)
			app["_include"] = includes
		}
		switch desc.Kind {
		case "app_field":
			delete(app, desc.Field)
		case "app_scalars_hash":
			delete(app, "type")
			delete(app, "immutable")
		}
	}
}

func yamlBody(v any) *string {
	if isBlankContainer(v) {
		return nil
	}
	b, err := yaml.Marshal(v)
	if err != nil {
		return nil
	}
	s := string(bytes.TrimSpace(b))
	s = strings.TrimPrefix(s, "---\n")
	s = strings.TrimSpace(s)
	if s == "" {
		return nil
	}
	return &s
}

func yamlBodySorted(v any) *string {
	return yamlBody(sortRec(v))
}

func canonicalJSON(m map[string]any) (string, error) {
	b, err := json.Marshal(sortRec(m))
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func sortRec(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(x))
		for _, k := range sortedKeys(x) {
			out[k] = sortRec(x[k])
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = sortRec(x[i])
		}
		return out
	default:
		return x
	}
}

func includeName(desc includeDescriptor, seq int) string {
	switch desc.Kind {
	case "app_field":
		field := strings.TrimSuffix(desc.Field, "YAML")
		return fmt.Sprintf("imported-%s-%d", camelToKebab(field), seq)
	case "app_scalars_hash":
		return fmt.Sprintf("imported-scalars-%d", seq)
	default:
		return fmt.Sprintf("imported-include-%d", seq)
	}
}

func genericAppKey(kind, ns, name, _ string) string {
	base := strings.TrimSpace(name)
	if base == "" {
		base = "resource"
	}
	prefix := camelToKebab(kind)
	if prefix == "" {
		prefix = "resource"
	}
	if strings.TrimSpace(ns) != "" {
		return sanitizeKey(prefix + "-" + ns + "-" + base)
	}
	return sanitizeKey(prefix + "-" + base)
}

func shortStableHash(v any) string {
	h := fnv.New32a()
	b, _ := json.Marshal(sortRec(v))
	_, _ = h.Write(b)
	return fmt.Sprintf("%08x", h.Sum32())[:6]
}

func camelToKebab(s string) string {
	var b strings.Builder
	for i, r := range s {
		if i > 0 && r >= 'A' && r <= 'Z' {
			b.WriteByte('-')
		}
		b.WriteRune(r)
	}
	return strings.ToLower(b.String())
}

func sanitizeKey(s string) string {
	s = strings.ToLower(s)
	var b strings.Builder
	prevDash := false
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
			prevDash = false
			continue
		}
		if !prevDash {
			b.WriteByte('-')
			prevDash = true
		}
	}
	out := strings.Trim(b.String(), "-")
	if out == "" {
		out = "item"
	}
	if len(out) > 63 {
		out = out[:63]
	}
	return out
}

func getMap(m map[string]any, key string) map[string]any {
	if v, ok := m[key].(map[string]any); ok {
		return v
	}
	return map[string]any{}
}

func mapStringAny(m map[string]any) map[string]any {
	out := make(map[string]any, len(m))
	for k, v := range m {
		out[k] = v
	}
	return out
}

func mapAny(v any) map[string]any {
	m, _ := v.(map[string]any)
	if m == nil {
		return map[string]any{}
	}
	return m
}

func cloneMap(m map[string]any) map[string]any {
	out := make(map[string]any, len(m))
	for k, v := range m {
		out[k] = cloneAny(v)
	}
	return out
}

func cloneAny(v any) any {
	switch x := v.(type) {
	case map[string]any:
		return cloneMap(x)
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = cloneAny(x[i])
		}
		return out
	default:
		return x
	}
}

func sortedKeys(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func sortedAppNames(group map[string]any) []string {
	var names []string
	for k := range group {
		if k == "__GroupVars__" {
			continue
		}
		names = append(names, k)
	}
	sort.Strings(names)
	return names
}

func uniqStrings(in []string) []string {
	seen := map[string]struct{}{}
	out := make([]string, 0, len(in))
	for _, s := range in {
		if _, ok := seen[s]; ok {
			continue
		}
		seen[s] = struct{}{}
		out = append(out, s)
	}
	return out
}

func containsStringAny(in []any, needle string) bool {
	for _, v := range in {
		if s, ok := v.(string); ok && s == needle {
			return true
		}
	}
	return false
}

func isBlankContainer(v any) bool {
	if v == nil {
		return true
	}
	switch x := v.(type) {
	case string:
		return strings.TrimSpace(x) == ""
	case []any:
		return len(x) == 0
	case map[string]any:
		return len(x) == 0
	default:
		return false
	}
}

func metadataName(doc map[string]any) string {
	meta := getMap(doc, "metadata")
	name, _ := meta["name"].(string)
	return strings.TrimSpace(name)
}

func metadataNamespace(doc map[string]any) string {
	meta := getMap(doc, "metadata")
	ns, _ := meta["namespace"].(string)
	return strings.TrimSpace(ns)
}

func builtinNamespaceAllowed(ns string) bool {
	ns = strings.TrimSpace(ns)
	return ns == "" || ns == "default"
}

func indentYAMLBlock(s string, spaces int) string {
	pad := strings.Repeat(" ", spaces)
	lines := strings.Split(strings.TrimRight(s, "\n"), "\n")
	for i := range lines {
		if strings.TrimSpace(lines[i]) == "" {
			lines[i] = pad
			continue
		}
		lines[i] = pad + lines[i]
	}
	return strings.Join(lines, "\n")
}

func filterImportedMetadataLabels(v any) any {
	labels, ok := v.(map[string]any)
	if !ok || len(labels) == 0 {
		return v
	}
	out := make(map[string]any, len(labels))
	for k, val := range labels {
		if _, drop := ignoredImportedMetadataLabelKeys[k]; drop {
			continue
		}
		out[k] = val
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func dedupeGroupKey(group map[string]any, base string) string {
	if _, exists := group[base]; !exists {
		return base
	}
	for i := 2; ; i++ {
		candidate := fmt.Sprintf("%s-%d", base, i)
		if _, exists := group[candidate]; !exists {
			return candidate
		}
	}
}

func mergeTopLevelValues(dst, src map[string]any) {
	for k, v := range src {
		if k == "global" {
			if dg, ok := dst["global"].(map[string]any); ok {
				if sg, ok := v.(map[string]any); ok {
					for gk, gv := range sg {
						if gk == "_includes" {
							if _, exists := dg[gk]; !exists {
								dg[gk] = gv
								continue
							}
							if dm, ok := dg[gk].(map[string]any); ok {
								if sm, ok := gv.(map[string]any); ok {
									for ik, iv := range sm {
										dm[ik] = iv
									}
								}
							}
							continue
						}
						dg[gk] = gv
					}
				}
			}
			continue
		}
		dst[k] = v
	}
}
