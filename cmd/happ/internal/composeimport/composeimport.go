package composeimport

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/zol/helm-apps/cmd/happ/internal/composeinspect"
	"github.com/zol/helm-apps/cmd/happ/internal/config"
	"gopkg.in/yaml.v3"
)

func BuildValues(cfg config.Config, rep composeinspect.Report) (map[string]any, []string, error) {
	values := map[string]any{
		"global": map[string]any{"env": cfg.Env},
	}
	apps := map[string]any{}
	warnings := append([]string{}, rep.Warnings...)

	for _, s := range rep.Services {
		appKey := sanitizeKey(s.Name)
		app := map[string]any{
			"enabled": true,
			"name":    s.Name,
		}
		container := map[string]any{}
		if img, ok := parseImage(s.Image); ok {
			container["image"] = img
		} else {
			warnings = append(warnings, fmt.Sprintf("service %s: image/build unsupported for MVP import (image=%q hasBuild=%v)", s.Name, s.Image, s.HasBuild))
			continue
		}
		if len(s.Entrypoint) > 0 {
			if y, err := yamlBlock(stringSliceToAny(s.Entrypoint)); err == nil {
				container["command"] = y
			}
		} else if strings.TrimSpace(s.EntrypointShell) != "" {
			if y, err := yamlBlock([]any{"/bin/sh", "-lc", s.EntrypointShell}); err == nil {
				container["command"] = y
			}
			warnings = append(warnings, fmt.Sprintf("service %s: shell-form entrypoint mapped to ['/bin/sh','-lc',...] (semantics may differ)", s.Name))
		}
		if len(s.Command) > 0 {
			if y, err := yamlBlock(stringSliceToAny(s.Command)); err == nil {
				container["args"] = y
			}
		} else if strings.TrimSpace(s.CommandShell) != "" {
			if y, err := yamlBlock([]any{"/bin/sh", "-lc", s.CommandShell}); err == nil {
				container["args"] = y
			}
			warnings = append(warnings, fmt.Sprintf("service %s: shell-form command mapped to ['/bin/sh','-lc',...] (semantics may differ)", s.Name))
		}
		if strings.TrimSpace(s.WorkingDir) != "" {
			container["workingDir"] = s.WorkingDir
		}
		if len(s.Env) > 0 {
			envVars := map[string]any{}
			for _, k := range sortedKeysStringMap(s.Env) {
				envVars[k] = s.Env[k]
			}
			container["envVars"] = envVars
		}
		if probe, warns := readinessProbeFromHealthcheck(s); len(probe) > 0 {
			container["readinessProbe"] = probe
			if cfg.ComposeHealthcheckToLiveness {
				container["livenessProbe"] = cloneMapAny(probe)
				warnings = append(warnings, fmt.Sprintf("service %s: compose healthcheck also mapped to livenessProbe (opt-in)", s.Name))
			}
			warnings = append(warnings, warns...)
		} else {
			warnings = append(warnings, warns...)
		}
		if len(s.PortsPublished) > 0 || len(s.Expose) > 0 {
			cports, svcPorts, warns := buildPorts(s)
			warnings = append(warnings, warns...)
			if len(cports) > 0 {
				if y, err := yamlBlock(cports); err == nil {
					container["ports"] = y
				}
			}
			if len(svcPorts) > 0 {
				service := map[string]any{"enabled": true}
				if y, err := yamlBlock(svcPorts); err == nil {
					service["ports"] = y
				}
				app["service"] = service
			}
		}
		if len(s.Labels) > 0 {
			if y, err := yamlBlock(stringMapToAny(s.Labels)); err == nil {
				app["labels"] = y
			}
		}
		if len(s.Profiles) > 0 {
			warnings = append(warnings, fmt.Sprintf("service %s: compose profiles not mapped in MVP import (%s)", s.Name, strings.Join(s.Profiles, ",")))
		}
		app["containers"] = map[string]any{"main": container}
		apps[dedupeKey(apps, appKey)] = app
	}
	if len(apps) == 0 {
		return nil, warnings, fmt.Errorf("no compose services could be mapped to apps-stateless")
	}
	values["apps-stateless"] = apps
	warnings = uniq(warnings)
	return values, warnings, nil
}

func readinessProbeFromHealthcheck(s composeinspect.ServiceNode) (map[string]any, []string) {
	h := s.Healthcheck
	if h == nil || h.Disable {
		return nil, nil
	}
	var cmd []any
	switch {
	case len(h.Test) > 0:
		cmd = stringSliceToAny(h.Test)
	case strings.TrimSpace(h.TestShell) != "":
		cmd = []any{"/bin/sh", "-lc", h.TestShell}
	default:
		return nil, nil
	}
	probe := map[string]any{
		"exec": map[string]any{
			"command": cmd,
		},
	}
	if h.TimeoutSeconds > 0 {
		probe["timeoutSeconds"] = h.TimeoutSeconds
	}
	if h.IntervalSeconds > 0 {
		probe["periodSeconds"] = h.IntervalSeconds
	}
	if h.StartPeriodSeconds > 0 {
		probe["initialDelaySeconds"] = h.StartPeriodSeconds
	}
	if h.Retries > 0 {
		probe["failureThreshold"] = h.Retries
	}
	var warns []string
	if strings.TrimSpace(h.TestShell) != "" {
		warns = append(warns, fmt.Sprintf("service %s: healthcheck shell-form mapped to readinessProbe.exec via /bin/sh -lc", s.Name))
	}
	return probe, warns
}

func cloneMapAny(in map[string]any) map[string]any {
	if in == nil {
		return nil
	}
	out := make(map[string]any, len(in))
	for k, v := range in {
		out[k] = cloneAny(v)
	}
	return out
}

func cloneAny(v any) any {
	switch x := v.(type) {
	case map[string]any:
		return cloneMapAny(x)
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = cloneAny(x[i])
		}
		return out
	default:
		return v
	}
}

func buildPorts(s composeinspect.ServiceNode) (containerPorts []any, servicePorts []any, warnings []string) {
	seenContainer := map[string]struct{}{}
	seenService := map[string]struct{}{}
	appendContainer := func(port string, proto string) {
		if strings.TrimSpace(port) == "" {
			return
		}
		k := port + "/" + strings.ToLower(proto)
		if _, ok := seenContainer[k]; ok {
			return
		}
		seenContainer[k] = struct{}{}
		m := map[string]any{"containerPort": toIntOrString(port)}
		if strings.TrimSpace(proto) != "" && !strings.EqualFold(proto, "tcp") {
			m["protocol"] = strings.ToUpper(proto)
		}
		containerPorts = append(containerPorts, m)
	}
	appendService := func(name, port, target, proto string) {
		if strings.TrimSpace(port) == "" && strings.TrimSpace(target) == "" {
			return
		}
		if strings.TrimSpace(port) == "" {
			port = target
		}
		if strings.TrimSpace(target) == "" {
			target = port
		}
		k := port + ":" + target + "/" + strings.ToLower(proto)
		if _, ok := seenService[k]; ok {
			return
		}
		seenService[k] = struct{}{}
		m := map[string]any{"port": toIntOrString(port), "targetPort": toIntOrString(target)}
		if name != "" {
			m["name"] = name
		}
		if strings.TrimSpace(proto) != "" && !strings.EqualFold(proto, "tcp") {
			m["protocol"] = strings.ToUpper(proto)
		}
		servicePorts = append(servicePorts, m)
	}

	idx := 0
	for _, p := range s.PortsPublished {
		idx++
		target, published, proto, ok := parseComposePortBinding(p)
		if !ok {
			warnings = append(warnings, fmt.Sprintf("service %s: unsupported port binding %q", s.Name, p.Raw))
			continue
		}
		appendContainer(target, proto)
		name := defaultPortName(target, idx)
		appendService(name, firstNonEmpty(published, target), target, proto)
	}
	for _, ex := range s.Expose {
		idx++
		port, proto := parseExpose(ex)
		if port == "" {
			continue
		}
		appendContainer(port, proto)
		appendService(defaultPortName(port, idx), port, port, proto)
	}
	return containerPorts, servicePorts, uniq(warnings)
}

func parseComposePortBinding(p composeinspect.PortBinding) (target, published, proto string, ok bool) {
	if p.Target != "" {
		return p.Target, p.Published, firstNonEmpty(strings.ToLower(p.Protocol), "tcp"), true
	}
	raw := strings.TrimSpace(p.Raw)
	if raw == "" {
		return "", "", "", false
	}
	proto = "tcp"
	if i := strings.LastIndex(raw, "/"); i > -1 {
		proto = strings.ToLower(strings.TrimSpace(raw[i+1:]))
		raw = raw[:i]
	}
	parts := strings.Split(raw, ":")
	if len(parts) == 1 {
		return parts[0], "", proto, true
	}
	// compose short syntax variants: ip:host:container or host:container
	target = strings.TrimSpace(parts[len(parts)-1])
	published = strings.TrimSpace(parts[len(parts)-2])
	return target, published, proto, target != ""
}

func parseExpose(v string) (port, proto string) {
	v = strings.TrimSpace(v)
	if v == "" {
		return "", ""
	}
	proto = "tcp"
	if i := strings.LastIndex(v, "/"); i > -1 {
		proto = strings.ToLower(strings.TrimSpace(v[i+1:]))
		v = strings.TrimSpace(v[:i])
	}
	return v, proto
}

func defaultPortName(port string, idx int) string {
	switch port {
	case "80":
		return "http"
	case "443":
		return "https"
	}
	p := strings.Map(func(r rune) rune {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') {
			return r
		}
		return '-'
	}, port)
	p = strings.Trim(strings.ToLower(p), "-")
	if p == "" {
		p = fmt.Sprintf("port-%d", idx)
	}
	if !strings.HasPrefix(p, "p") && strings.IndexFunc(p, func(r rune) bool { return r < '0' || r > '9' }) == -1 {
		p = "p" + p
	}
	return p
}

func parseImage(s string) (map[string]any, bool) {
	s = strings.TrimSpace(s)
	if s == "" {
		return nil, false
	}
	lastSlash := strings.LastIndex(s, "/")
	lastColon := strings.LastIndex(s, ":")
	if lastColon <= lastSlash {
		return map[string]any{"name": s, "staticTag": "latest"}, true
	}
	return map[string]any{"name": s[:lastColon], "staticTag": s[lastColon+1:]}, true
}

func yamlBlock(v any) (string, error) {
	b, err := yaml.Marshal(v)
	if err != nil {
		return "", err
	}
	return strings.TrimRight(string(b), "\n"), nil
}

func toIntOrString(s string) any {
	s = strings.TrimSpace(s)
	if s == "" {
		return ""
	}
	if i, err := strconv.Atoi(s); err == nil {
		return i
	}
	return s
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if strings.TrimSpace(v) != "" {
			return strings.TrimSpace(v)
		}
	}
	return ""
}

func stringMapToAny(m map[string]string) map[string]any {
	out := map[string]any{}
	for _, k := range sortedKeysStringMap(m) {
		out[k] = m[k]
	}
	return out
}

func stringSliceToAny(in []string) []any {
	out := make([]any, 0, len(in))
	for _, s := range in {
		out = append(out, s)
	}
	return out
}

func sortedKeysStringMap(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func sanitizeKey(s string) string {
	s = strings.ToLower(strings.TrimSpace(s))
	if s == "" {
		return "app"
	}
	var b strings.Builder
	prevDash := false
	for _, r := range s {
		ok := (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' || r == '.'
		if !ok {
			r = '-'
		}
		if r == '-' {
			if prevDash {
				continue
			}
			prevDash = true
		} else {
			prevDash = false
		}
		b.WriteRune(r)
	}
	res := strings.Trim(b.String(), "-.")
	if res == "" {
		return "app"
	}
	return res
}

func dedupeKey(group map[string]any, key string) string {
	if _, ok := group[key]; !ok {
		return key
	}
	for i := 2; ; i++ {
		k := fmt.Sprintf("%s-%d", key, i)
		if _, ok := group[k]; !ok {
			return k
		}
	}
}

func uniq(in []string) []string {
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
	sort.Strings(out)
	return out
}
