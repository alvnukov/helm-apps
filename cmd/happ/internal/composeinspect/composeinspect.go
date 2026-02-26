package composeinspect

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

type Report struct {
	SourcePath string        `json:"sourcePath" yaml:"sourcePath"`
	Project    string        `json:"project,omitempty" yaml:"project,omitempty"`
	Services   []ServiceNode `json:"services" yaml:"services"`
	Resources  []NamedNode   `json:"resources,omitempty" yaml:"resources,omitempty"`
	Relations  []Relation    `json:"relations" yaml:"relations"`
	Summary    Summary       `json:"summary" yaml:"summary"`
	Warnings   []string      `json:"warnings,omitempty" yaml:"warnings,omitempty"`
}

type Summary struct {
	ServiceCount  int `json:"serviceCount" yaml:"serviceCount"`
	ResourceCount int `json:"resourceCount" yaml:"resourceCount"`
	RelationCount int `json:"relationCount" yaml:"relationCount"`
}

type ServiceNode struct {
	ID              string            `json:"id" yaml:"id"`
	Name            string            `json:"name" yaml:"name"`
	Image           string            `json:"image,omitempty" yaml:"image,omitempty"`
	HasBuild        bool              `json:"hasBuild,omitempty" yaml:"hasBuild,omitempty"`
	Command         []string          `json:"command,omitempty" yaml:"command,omitempty"`
	CommandShell    string            `json:"commandShell,omitempty" yaml:"commandShell,omitempty"`
	Entrypoint      []string          `json:"entrypoint,omitempty" yaml:"entrypoint,omitempty"`
	EntrypointShell string            `json:"entrypointShell,omitempty" yaml:"entrypointShell,omitempty"`
	WorkingDir      string            `json:"workingDir,omitempty" yaml:"workingDir,omitempty"`
	Healthcheck     *Healthcheck      `json:"healthcheck,omitempty" yaml:"healthcheck,omitempty"`
	Profiles        []string          `json:"profiles,omitempty" yaml:"profiles,omitempty"`
	DependsOn       []string          `json:"dependsOn,omitempty" yaml:"dependsOn,omitempty"`
	PortsPublished  []PortBinding     `json:"portsPublished,omitempty" yaml:"portsPublished,omitempty"`
	Expose          []string          `json:"expose,omitempty" yaml:"expose,omitempty"`
	Env             map[string]string `json:"env,omitempty" yaml:"env,omitempty"`
	EnvFiles        []string          `json:"envFiles,omitempty" yaml:"envFiles,omitempty"`
	Networks        []string          `json:"networks,omitempty" yaml:"networks,omitempty"`
	Volumes         []MountRef        `json:"volumes,omitempty" yaml:"volumes,omitempty"`
	Configs         []Ref             `json:"configs,omitempty" yaml:"configs,omitempty"`
	Secrets         []Ref             `json:"secrets,omitempty" yaml:"secrets,omitempty"`
	Labels          map[string]string `json:"labels,omitempty" yaml:"labels,omitempty"`
	RoleHints       []string          `json:"roleHints,omitempty" yaml:"roleHints,omitempty"`
}

type Healthcheck struct {
	Disable            bool     `json:"disable,omitempty" yaml:"disable,omitempty"`
	Test               []string `json:"test,omitempty" yaml:"test,omitempty"`
	TestShell          string   `json:"testShell,omitempty" yaml:"testShell,omitempty"`
	IntervalSeconds    int      `json:"intervalSeconds,omitempty" yaml:"intervalSeconds,omitempty"`
	TimeoutSeconds     int      `json:"timeoutSeconds,omitempty" yaml:"timeoutSeconds,omitempty"`
	StartPeriodSeconds int      `json:"startPeriodSeconds,omitempty" yaml:"startPeriodSeconds,omitempty"`
	Retries            int      `json:"retries,omitempty" yaml:"retries,omitempty"`
}

type PortBinding struct {
	Published string `json:"published,omitempty" yaml:"published,omitempty"`
	Target    string `json:"target,omitempty" yaml:"target,omitempty"`
	Protocol  string `json:"protocol,omitempty" yaml:"protocol,omitempty"`
	HostIP    string `json:"hostIp,omitempty" yaml:"hostIp,omitempty"`
	Mode      string `json:"mode,omitempty" yaml:"mode,omitempty"`
	Raw       string `json:"raw,omitempty" yaml:"raw,omitempty"`
}

type MountRef struct {
	Kind     string `json:"kind" yaml:"kind"` // bind|volume|tmpfs|unknown
	Source   string `json:"source,omitempty" yaml:"source,omitempty"`
	Target   string `json:"target,omitempty" yaml:"target,omitempty"`
	ReadOnly bool   `json:"readOnly,omitempty" yaml:"readOnly,omitempty"`
	Raw      string `json:"raw,omitempty" yaml:"raw,omitempty"`
}

type Ref struct {
	Source   string `json:"source,omitempty" yaml:"source,omitempty"`
	Target   string `json:"target,omitempty" yaml:"target,omitempty"`
	ReadOnly bool   `json:"readOnly,omitempty" yaml:"readOnly,omitempty"`
}

type NamedNode struct {
	ID   string `json:"id" yaml:"id"`
	Kind string `json:"kind" yaml:"kind"` // volume|network|secret|config
	Name string `json:"name" yaml:"name"`
}

type Relation struct {
	From   string `json:"from" yaml:"from"`
	To     string `json:"to" yaml:"to"`
	Type   string `json:"type" yaml:"type"`
	Detail string `json:"detail,omitempty" yaml:"detail,omitempty"`
}

type composeFile struct {
	Name     string         `yaml:"name"`
	Services map[string]any `yaml:"services"`
	Volumes  map[string]any `yaml:"volumes"`
	Networks map[string]any `yaml:"networks"`
	Secrets  map[string]any `yaml:"secrets"`
	Configs  map[string]any `yaml:"configs"`
}

func Load(path string) (Report, error) {
	p, err := resolveComposePath(path)
	if err != nil {
		return Report{}, err
	}
	b, err := os.ReadFile(p)
	if err != nil {
		return Report{}, err
	}
	var cf composeFile
	if err := yaml.Unmarshal(b, &cf); err != nil {
		return Report{}, fmt.Errorf("parse compose yaml: %w", err)
	}
	if len(cf.Services) == 0 {
		return Report{}, fmt.Errorf("no services found in compose file")
	}
	return buildReport(p, cf), nil
}

func ResolveAndWrite(path, format, out string) error {
	rep, err := Load(path)
	if err != nil {
		return err
	}
	var b []byte
	switch strings.ToLower(strings.TrimSpace(format)) {
	case "", "yaml", "yml":
		b, err = yaml.Marshal(rep)
	case "json":
		b, err = json.MarshalIndent(rep, "", "  ")
	default:
		return fmt.Errorf("unsupported format %q (expected yaml or json)", format)
	}
	if err != nil {
		return err
	}
	if out == "" {
		_, err = os.Stdout.Write(b)
		if err == nil && (len(b) == 0 || b[len(b)-1] != '\n') {
			_, _ = os.Stdout.Write([]byte("\n"))
		}
		return err
	}
	return os.WriteFile(out, b, 0o644)
}

func resolveComposePath(path string) (string, error) {
	st, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !st.IsDir() {
		return path, nil
	}
	candidates := []string{"compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"}
	for _, c := range candidates {
		p := filepath.Join(path, c)
		if _, err := os.Stat(p); err == nil {
			return p, nil
		}
	}
	return "", fmt.Errorf("compose file not found in directory %s", path)
}

func buildReport(path string, cf composeFile) Report {
	serviceNames := sortedKeysAny(cf.Services)
	services := make([]ServiceNode, 0, len(serviceNames))
	serviceSet := map[string]struct{}{}
	for _, n := range serviceNames {
		serviceSet[n] = struct{}{}
	}

	relations := []Relation{}
	resourceNodes := map[string]NamedNode{}
	warnings := []string{}

	for _, svcName := range serviceNames {
		raw := asMap(cf.Services[svcName])
		if raw == nil {
			warnings = append(warnings, fmt.Sprintf("service %s is not a map", svcName))
			continue
		}
		svc := ServiceNode{ID: "service:" + svcName, Name: svcName}
		svc.Image = asString(raw["image"])
		_, svc.HasBuild = raw["build"]
		svc.Command, svc.CommandShell = parseCommandLike(raw["command"])
		svc.Entrypoint, svc.EntrypointShell = parseCommandLike(raw["entrypoint"])
		svc.WorkingDir = asString(raw["working_dir"])
		svc.Healthcheck = parseHealthcheck(raw["healthcheck"])
		svc.Profiles = parseStringListOrMapKeys(raw["profiles"])
		svc.DependsOn = parseDependsOn(raw["depends_on"])
		svc.PortsPublished = parsePorts(raw["ports"])
		svc.Expose = parseStringList(raw["expose"])
		svc.Env = parseEnvironment(raw["environment"])
		svc.EnvFiles = parseEnvFiles(raw["env_file"])
		svc.Networks = parseNetworks(raw["networks"])
		svc.Volumes = parseMounts(raw["volumes"])
		svc.Configs = parseRefs(raw["configs"])
		svc.Secrets = parseRefs(raw["secrets"])
		svc.Labels = parseStringMapOrList(raw["labels"])
		svc.RoleHints = detectRoleHints(svc)
		services = append(services, svc)

		for _, dep := range svc.DependsOn {
			relations = append(relations, Relation{From: svc.ID, To: "service:" + dep, Type: "depends_on"})
		}
		for k, v := range svc.Env {
			for _, target := range inferServiceRefsFromEnvValue(v, serviceSet) {
				relations = append(relations, Relation{From: svc.ID, To: "service:" + target, Type: "env_ref", Detail: k})
			}
		}
		for _, netName := range svc.Networks {
			if netName == "" {
				continue
			}
			id := "network:" + netName
			resourceNodes[id] = NamedNode{ID: id, Kind: "network", Name: netName}
			relations = append(relations, Relation{From: svc.ID, To: id, Type: "uses_network"})
		}
		for _, m := range svc.Volumes {
			if m.Kind == "volume" && m.Source != "" {
				id := "volume:" + m.Source
				resourceNodes[id] = NamedNode{ID: id, Kind: "volume", Name: m.Source}
				relations = append(relations, Relation{From: svc.ID, To: id, Type: "mounts_volume", Detail: m.Target})
			} else if m.Kind == "bind" {
				relations = append(relations, Relation{From: svc.ID, To: "bind:" + m.Source, Type: "bind_mount", Detail: m.Target})
			}
		}
		for _, c := range svc.Configs {
			if c.Source == "" {
				continue
			}
			id := "config:" + c.Source
			resourceNodes[id] = NamedNode{ID: id, Kind: "config", Name: c.Source}
			relations = append(relations, Relation{From: svc.ID, To: id, Type: "uses_config", Detail: c.Target})
		}
		for _, s := range svc.Secrets {
			if s.Source == "" {
				continue
			}
			id := "secret:" + s.Source
			resourceNodes[id] = NamedNode{ID: id, Kind: "secret", Name: s.Source}
			relations = append(relations, Relation{From: svc.ID, To: id, Type: "uses_secret", Detail: s.Target})
		}
	}

	for name := range cf.Volumes {
		resourceNodes["volume:"+name] = NamedNode{ID: "volume:" + name, Kind: "volume", Name: name}
	}
	for name := range cf.Networks {
		resourceNodes["network:"+name] = NamedNode{ID: "network:" + name, Kind: "network", Name: name}
	}
	for name := range cf.Secrets {
		resourceNodes["secret:"+name] = NamedNode{ID: "secret:" + name, Kind: "secret", Name: name}
	}
	for name := range cf.Configs {
		resourceNodes["config:"+name] = NamedNode{ID: "config:" + name, Kind: "config", Name: name}
	}

	sort.Slice(services, func(i, j int) bool { return services[i].Name < services[j].Name })
	resources := make([]NamedNode, 0, len(resourceNodes))
	for _, n := range resourceNodes {
		resources = append(resources, n)
	}
	sort.Slice(resources, func(i, j int) bool {
		if resources[i].Kind != resources[j].Kind {
			return resources[i].Kind < resources[j].Kind
		}
		return resources[i].Name < resources[j].Name
	})
	sort.Slice(relations, func(i, j int) bool {
		if relations[i].From != relations[j].From {
			return relations[i].From < relations[j].From
		}
		if relations[i].To != relations[j].To {
			return relations[i].To < relations[j].To
		}
		if relations[i].Type != relations[j].Type {
			return relations[i].Type < relations[j].Type
		}
		return relations[i].Detail < relations[j].Detail
	})
	warnings = uniqStrings(warnings)
	return Report{
		SourcePath: path,
		Project:    cf.Name,
		Services:   services,
		Resources:  resources,
		Relations:  relations,
		Warnings:   warnings,
		Summary:    Summary{ServiceCount: len(services), ResourceCount: len(resources), RelationCount: len(relations)},
	}
}

func asMap(v any) map[string]any {
	if m, ok := v.(map[string]any); ok {
		return m
	}
	return nil
}

func asString(v any) string { s, _ := v.(string); return strings.TrimSpace(s) }

func parseDependsOn(v any) []string {
	switch x := v.(type) {
	case []any:
		return uniqStrings(parseStringList(v))
	case map[string]any:
		keys := sortedKeysAny(x)
		return uniqStrings(keys)
	default:
		return nil
	}
}

func parseEnvironment(v any) map[string]string {
	out := map[string]string{}
	switch x := v.(type) {
	case map[string]any:
		for _, k := range sortedKeysAny(x) {
			out[k] = stringifyScalar(x[k])
		}
	case []any:
		for _, it := range x {
			s := asString(it)
			if s == "" {
				continue
			}
			if idx := strings.Index(s, "="); idx >= 0 {
				out[s[:idx]] = s[idx+1:]
			} else {
				out[s] = ""
			}
		}
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseEnvFiles(v any) []string {
	if s := asString(v); s != "" {
		return []string{s}
	}
	return uniqStrings(parseStringList(v))
}

func parsePorts(v any) []PortBinding {
	items, _ := v.([]any)
	out := make([]PortBinding, 0, len(items))
	for _, it := range items {
		s := asString(it)
		if s != "" {
			out = append(out, PortBinding{Raw: s})
			continue
		}
		m := asMap(it)
		if m == nil {
			continue
		}
		pb := PortBinding{Published: stringifyScalar(m["published"]), Target: stringifyScalar(m["target"]), Protocol: asString(m["protocol"]), HostIP: asString(m["host_ip"]), Mode: asString(m["mode"])}
		out = append(out, pb)
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseMounts(v any) []MountRef {
	items, _ := v.([]any)
	out := make([]MountRef, 0, len(items))
	for _, it := range items {
		if s := asString(it); s != "" {
			out = append(out, parseShortMount(s))
			continue
		}
		m := asMap(it)
		if m == nil {
			continue
		}
		mr := MountRef{Kind: asString(m["type"]), Source: asString(m["source"]), Target: asString(m["target"]), ReadOnly: isTrue(m["read_only"])}
		if mr.Kind == "" {
			mr.Kind = "unknown"
		}
		out = append(out, mr)
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseShortMount(s string) MountRef {
	parts := strings.Split(s, ":")
	mr := MountRef{Raw: s}
	switch len(parts) {
	case 1:
		mr.Kind = "volume"
		mr.Target = parts[0]
	case 2, 3:
		mr.Source = parts[0]
		mr.Target = parts[1]
		if len(parts) == 3 && strings.Contains(parts[2], "ro") {
			mr.ReadOnly = true
		}
		if strings.HasPrefix(mr.Source, "/") || strings.HasPrefix(mr.Source, "./") || strings.HasPrefix(mr.Source, "../") || strings.Contains(mr.Source, string(os.PathSeparator)) {
			mr.Kind = "bind"
		} else {
			mr.Kind = "volume"
		}
	default:
		mr.Kind = "unknown"
	}
	return mr
}

func parseRefs(v any) []Ref {
	items, _ := v.([]any)
	out := make([]Ref, 0, len(items))
	for _, it := range items {
		if s := asString(it); s != "" {
			out = append(out, Ref{Source: s})
			continue
		}
		m := asMap(it)
		if m == nil {
			continue
		}
		out = append(out, Ref{Source: asString(m["source"]), Target: asString(m["target"]), ReadOnly: isTrue(m["read_only"])})
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseNetworks(v any) []string {
	switch x := v.(type) {
	case []any:
		return uniqStrings(parseStringList(v))
	case map[string]any:
		return uniqStrings(sortedKeysAny(x))
	default:
		return nil
	}
}

func parseStringMapOrList(v any) map[string]string {
	out := map[string]string{}
	switch x := v.(type) {
	case map[string]any:
		for _, k := range sortedKeysAny(x) {
			out[k] = stringifyScalar(x[k])
		}
	case []any:
		for _, it := range x {
			s := asString(it)
			if s == "" {
				continue
			}
			if idx := strings.Index(s, "="); idx > 0 {
				out[s[:idx]] = s[idx+1:]
			}
		}
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseStringListOrMapKeys(v any) []string {
	if m, ok := v.(map[string]any); ok {
		return sortedKeysAny(m)
	}
	return parseStringList(v)
}

func parseStringList(v any) []string {
	items, _ := v.([]any)
	out := []string{}
	for _, it := range items {
		if s := stringifyScalar(it); strings.TrimSpace(s) != "" {
			out = append(out, strings.TrimSpace(s))
		}
	}
	return uniqStrings(out)
}

func parseCommandLike(v any) (list []string, shell string) {
	if s := asString(v); s != "" {
		return nil, s
	}
	out := parseStringList(v)
	if len(out) == 0 {
		return nil, ""
	}
	return out, ""
}

func parseHealthcheck(v any) *Healthcheck {
	m := asMap(v)
	if m == nil {
		return nil
	}
	h := &Healthcheck{}
	if isTrue(m["disable"]) {
		h.Disable = true
		return h
	}
	h.IntervalSeconds = parseComposeDurationSeconds(m["interval"])
	h.TimeoutSeconds = parseComposeDurationSeconds(m["timeout"])
	h.StartPeriodSeconds = parseComposeDurationSeconds(m["start_period"])
	if x, err := strconv.Atoi(strings.TrimSpace(stringifyScalar(m["retries"]))); err == nil && x > 0 {
		h.Retries = x
	}
	switch t := m["test"].(type) {
	case []any:
		items := parseOrderedStringList(t)
		if len(items) == 0 {
			return h
		}
		if len(items) == 1 && strings.EqualFold(strings.TrimSpace(items[0]), "NONE") {
			h.Disable = true
			h.Test = nil
			return h
		}
		// Compose semantics:
		// ["CMD", ...] -> exec form
		// ["CMD-SHELL", "..."] -> shell form
		if len(items) >= 1 && strings.EqualFold(items[0], "CMD-SHELL") {
			if len(items) >= 2 {
				h.TestShell = items[1]
			}
			return h
		}
		if len(items) >= 1 && strings.EqualFold(items[0], "CMD") {
			h.Test = append([]string{}, items[1:]...)
			return h
		}
		h.Test = items
	case string:
		s := strings.TrimSpace(t)
		if s == "" {
			return h
		}
		if strings.EqualFold(s, "NONE") {
			h.Disable = true
			return h
		}
		h.TestShell = s
	default:
	}
	if h.Disable || h.TestShell != "" || len(h.Test) > 0 || h.IntervalSeconds > 0 || h.TimeoutSeconds > 0 || h.StartPeriodSeconds > 0 || h.Retries > 0 {
		return h
	}
	return nil
}

func parseOrderedStringList(v any) []string {
	items, _ := v.([]any)
	out := make([]string, 0, len(items))
	for _, it := range items {
		s := stringifyScalar(it)
		s = strings.TrimSpace(s)
		if s == "" {
			continue
		}
		out = append(out, s)
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func parseComposeDurationSeconds(v any) int {
	s := strings.TrimSpace(stringifyScalar(v))
	if s == "" {
		return 0
	}
	if i, err := strconv.Atoi(s); err == nil && i >= 0 {
		return i
	}
	d, err := time.ParseDuration(s)
	if err != nil || d <= 0 {
		return 0
	}
	sec := int(d / time.Second)
	if sec <= 0 && d > 0 {
		sec = 1
	}
	return sec
}

func inferServiceRefsFromEnvValue(v string, serviceSet map[string]struct{}) []string {
	v = strings.TrimSpace(v)
	if v == "" {
		return nil
	}
	seen := map[string]struct{}{}
	for name := range serviceSet {
		if v == name || strings.Contains(v, "://"+name) || strings.Contains(v, "@"+name) || strings.Contains(v, "host="+name) {
			seen[name] = struct{}{}
		}
	}
	out := make([]string, 0, len(seen))
	for s := range seen {
		out = append(out, s)
	}
	sort.Strings(out)
	return out
}

func detectRoleHints(s ServiceNode) []string {
	hints := []string{}
	name := strings.ToLower(s.Name)
	img := strings.ToLower(s.Image)
	labelsText := strings.ToLower(strings.Join(mapValues(s.Labels), " "))
	if containsAnySubstr([]string{name, img, labelsText}, "postgres", "mysql", "mariadb", "mongo") {
		hints = append(hints, "database")
	}
	if containsAnySubstr([]string{name, img, labelsText}, "redis", "memcached") {
		hints = append(hints, "cache")
	}
	if containsAnySubstr([]string{name, img, labelsText}, "nginx", "traefik", "haproxy", "caddy") {
		hints = append(hints, "proxy")
	}
	if containsAnySubstr([]string{name, labelsText}, "worker", "queue") {
		hints = append(hints, "worker")
	}
	if containsAnySubstr([]string{name, labelsText}, "cron") {
		hints = append(hints, "cron")
	}
	if len(s.PortsPublished) > 0 {
		hints = append(hints, "published-port")
	}
	if len(hints) == 0 {
		return nil
	}
	return uniqStrings(hints)
}

func containsAnySubstr(haystacks []string, needles ...string) bool {
	for _, h := range haystacks {
		h = strings.ToLower(h)
		if strings.TrimSpace(h) == "" {
			continue
		}
		for _, n := range needles {
			n = strings.ToLower(strings.TrimSpace(n))
			if n == "" {
				continue
			}
			if strings.Contains(h, n) {
				return true
			}
		}
	}
	return false
}

func stringifyScalar(v any) string {
	switch x := v.(type) {
	case nil:
		return ""
	case string:
		return x
	case int:
		return strconv.Itoa(x)
	case int64:
		return strconv.FormatInt(x, 10)
	case float64:
		if x == float64(int64(x)) {
			return strconv.FormatInt(int64(x), 10)
		}
		return strconv.FormatFloat(x, 'f', -1, 64)
	case bool:
		if x {
			return "true"
		}
		return "false"
	default:
		b, _ := json.Marshal(x)
		return string(b)
	}
}

func isTrue(v any) bool {
	if b, ok := v.(bool); ok {
		return b
	}
	if s, ok := v.(string); ok {
		s = strings.ToLower(strings.TrimSpace(s))
		return s == "true" || s == "1" || s == "yes" || s == "on"
	}
	return false
}

func sortedKeysAny(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func uniqStrings(in []string) []string {
	if len(in) == 0 {
		return nil
	}
	seen := map[string]struct{}{}
	out := make([]string, 0, len(in))
	for _, s := range in {
		s = strings.TrimSpace(s)
		if s == "" {
			continue
		}
		if _, ok := seen[s]; ok {
			continue
		}
		seen[s] = struct{}{}
		out = append(out, s)
	}
	if len(out) == 0 {
		return nil
	}
	sort.Strings(out)
	return out
}

func mapValues(m map[string]string) []string {
	if len(m) == 0 {
		return nil
	}
	out := make([]string, 0, len(m))
	for _, v := range m {
		out = append(out, v)
	}
	return out
}
