package dyfflike

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"reflect"
	"sort"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

// Options controls semantic diff behavior.
type Options struct {
	IgnoreOrderChanges     bool
	IgnoreWhitespaceChange bool
	// AdditionalIdentifiers are dot-separated relative field paths for list item matching (e.g. "id", "meta.name").
	AdditionalIdentifiers []string
}

type changeKind string

const (
	changeAdded   changeKind = "added"
	changeRemoved changeKind = "removed"
	changeChanged changeKind = "changed"
)

type change struct {
	Kind  changeKind
	Path  string
	Left  any
	Right any
}

var defaultOptions = Options{
	IgnoreOrderChanges: true,
	AdditionalIdentifiers: []string{
		"metadata.name",
		"name",
		"id",
		"key",
		"mountPath",
		"path",
		"containerPort",
		"port",
	},
}

// BetweenDocs renders a deterministic, human-readable diff between two YAML-like docs.
// Empty string means semantically equal for this formatter.
func BetweenDocs(from, to map[string]any) (string, error) {
	return BetweenValuesWithOptions(from, to, defaultOptions)
}

// BetweenYAML parses YAML (single or multi-doc stream) and renders a deterministic diff.
func BetweenYAML(fromYAML, toYAML []byte) (string, error) {
	return BetweenYAMLWithOptions(fromYAML, toYAML, defaultOptions)
}

// BetweenYAMLWithOptions parses YAML (single or multi-doc stream) and renders a deterministic diff.
func BetweenYAMLWithOptions(fromYAML, toYAML []byte, opts Options) (string, error) {
	from, err := parseYAMLAny(fromYAML)
	if err != nil {
		return "", fmt.Errorf("parse source yaml: %w", err)
	}
	to, err := parseYAMLAny(toYAML)
	if err != nil {
		return "", fmt.Errorf("parse generated yaml: %w", err)
	}
	return BetweenValuesWithOptions(from, to, opts)
}

// BetweenValues compares arbitrary normalized values and returns a stable text diff.
func BetweenValues(from, to any) (string, error) {
	return BetweenValuesWithOptions(from, to, defaultOptions)
}

// BetweenValuesWithOptions compares arbitrary normalized values and returns a stable text diff.
func BetweenValuesWithOptions(from, to any, opts Options) (string, error) {
	from = normalizeYAMLValue(from)
	to = normalizeYAMLValue(to)
	changes := make([]change, 0, 8)
	collectDiffs("$", from, to, &changes, opts)
	if len(changes) == 0 {
		return "", nil
	}
	return formatChanges(changes), nil
}

func parseYAMLAny(b []byte) (any, error) {
	dec := yaml.NewDecoder(bytes.NewReader(b))
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
		docs = append(docs, normalizeYAMLValue(v))
	}
	if len(docs) == 0 {
		return []any{}, nil
	}
	if len(docs) == 1 {
		return docs[0], nil
	}
	return docs, nil
}

func normalizeYAMLValue(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[k] = normalizeYAMLValue(vv)
		}
		return out
	case map[any]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[fmt.Sprint(k)] = normalizeYAMLValue(vv)
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = normalizeYAMLValue(x[i])
		}
		return out
	default:
		return x
	}
}

func collectDiffs(path string, left, right any, out *[]change, opts Options) {
	switch l := left.(type) {
	case map[string]any:
		r, ok := right.(map[string]any)
		if !ok {
			*out = append(*out, change{Kind: changeChanged, Path: path, Left: left, Right: right})
			return
		}
		keysSet := map[string]struct{}{}
		for k := range l {
			keysSet[k] = struct{}{}
		}
		for k := range r {
			keysSet[k] = struct{}{}
		}
		keys := make([]string, 0, len(keysSet))
		for k := range keysSet {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			lv, lok := l[k]
			rv, rok := r[k]
			nextPath := path + "." + k
			switch {
			case !lok && rok:
				*out = append(*out, change{Kind: changeAdded, Path: nextPath, Right: rv})
			case lok && !rok:
				*out = append(*out, change{Kind: changeRemoved, Path: nextPath, Left: lv})
			default:
				collectDiffs(nextPath, lv, rv, out, opts)
			}
		}
	case []any:
		r, ok := right.([]any)
		if !ok {
			*out = append(*out, change{Kind: changeChanged, Path: path, Left: left, Right: right})
			return
		}
		if opts.IgnoreOrderChanges && collectListDiffsIgnoringOrder(path, l, r, out, opts) {
			return
		}
		if len(l) != len(r) {
			*out = append(*out, change{Kind: changeChanged, Path: path + ".length", Left: len(l), Right: len(r)})
		}
		n := len(l)
		if len(r) < n {
			n = len(r)
		}
		for i := 0; i < n; i++ {
			collectDiffs(path+"["+strconv.Itoa(i)+"]", l[i], r[i], out, opts)
		}
		for i := n; i < len(l); i++ {
			*out = append(*out, change{Kind: changeRemoved, Path: path + "[" + strconv.Itoa(i) + "]", Left: l[i]})
		}
		for i := n; i < len(r); i++ {
			*out = append(*out, change{Kind: changeAdded, Path: path + "[" + strconv.Itoa(i) + "]", Right: r[i]})
		}
	default:
		if !scalarEqual(left, right, opts) {
			*out = append(*out, change{Kind: changeChanged, Path: path, Left: left, Right: right})
		}
	}
}

func scalarEqual(a, b any, opts Options) bool {
	if opts.IgnoreWhitespaceChange {
		as, aok := a.(string)
		bs, bok := b.(string)
		if aok && bok && strings.TrimSpace(as) == strings.TrimSpace(bs) {
			return true
		}
	}
	aj, aerr := json.Marshal(a)
	bj, berr := json.Marshal(b)
	if aerr == nil && berr == nil {
		return bytes.Equal(aj, bj)
	}
	return fmt.Sprintf("%v", a) == fmt.Sprintf("%v", b)
}

func collectListDiffsIgnoringOrder(path string, left, right []any, out *[]change, opts Options) bool {
	if len(left) == 0 && len(right) == 0 {
		return true
	}
	if identPath, ids := detectIdentifierPath(left, right, opts); identPath != "" {
		leftIdx := make(map[string]any, len(left))
		rightIdx := make(map[string]any, len(right))
		for i, id := range ids.leftIDs {
			leftIdx[id] = left[i]
		}
		for i, id := range ids.rightIDs {
			rightIdx[id] = right[i]
		}
		allIDs := make([]string, 0, len(leftIdx)+len(rightIdx))
		seen := map[string]struct{}{}
		for id := range leftIdx {
			seen[id] = struct{}{}
			allIDs = append(allIDs, id)
		}
		for id := range rightIdx {
			if _, ok := seen[id]; !ok {
				allIDs = append(allIDs, id)
			}
		}
		sort.Strings(allIDs)
		for _, id := range allIDs {
			lv, lok := leftIdx[id]
			rv, rok := rightIdx[id]
			nextPath := fmt.Sprintf("%s[%s=%s]", path, identPath, escapePathSegment(id))
			switch {
			case !lok && rok:
				*out = append(*out, change{Kind: changeAdded, Path: nextPath, Right: rv})
			case lok && !rok:
				*out = append(*out, change{Kind: changeRemoved, Path: nextPath, Left: lv})
			default:
				collectDiffs(nextPath, lv, rv, out, opts)
			}
		}
		return true
	}
	if multisetEqual(left, right, opts) {
		return true
	}
	return false
}

type detectedIDs struct {
	leftIDs  []string
	rightIDs []string
}

func detectIdentifierPath(left, right []any, opts Options) (string, detectedIDs) {
	if leftIDs, rightIDs, ok := detectK8sDocIDs(left, right); ok {
		return "k8s", detectedIDs{leftIDs: leftIDs, rightIDs: rightIDs}
	}
	candidates := listIdentifierCandidates(opts)
	for _, candidate := range candidates {
		leftIDs, okL := extractUniqueIDs(left, candidate)
		rightIDs, okR := extractUniqueIDs(right, candidate)
		if okL && okR {
			return candidate, detectedIDs{leftIDs: leftIDs, rightIDs: rightIDs}
		}
	}
	return "", detectedIDs{}
}

func detectK8sDocIDs(left, right []any) ([]string, []string, bool) {
	leftIDs, okL := extractK8sDocIDs(left)
	rightIDs, okR := extractK8sDocIDs(right)
	if okL && okR {
		return leftIDs, rightIDs, true
	}
	return nil, nil, false
}

func extractK8sDocIDs(items []any) ([]string, bool) {
	ids := make([]string, 0, len(items))
	seen := map[string]struct{}{}
	for _, item := range items {
		m, ok := item.(map[string]any)
		if !ok {
			return nil, false
		}
		apiVersion, _ := m["apiVersion"].(string)
		kind, _ := m["kind"].(string)
		meta, _ := m["metadata"].(map[string]any)
		if apiVersion == "" || kind == "" || meta == nil {
			return nil, false
		}
		name, _ := meta["name"].(string)
		if name == "" {
			return nil, false
		}
		ns, _ := meta["namespace"].(string)
		if ns == "" {
			ns = "default"
		}
		id := apiVersion + "|" + kind + "|" + ns + "|" + name
		if _, dup := seen[id]; dup {
			return nil, false
		}
		seen[id] = struct{}{}
		ids = append(ids, id)
	}
	return ids, true
}

func listIdentifierCandidates(opts Options) []string {
	candidates := []string{
		"metadata.name",
		// K8s-ish refs used in envFrom-like structures.
		"configMapRef.name",
		"secretRef.name",
		// Common fields.
		"name",
		"id",
		"key",
		"mountPath",
		"path",
		"containerPort",
		"port",
	}
	candidates = append(candidates, opts.AdditionalIdentifiers...)
	// Dedupe preserving order.
	seen := map[string]struct{}{}
	out := make([]string, 0, len(candidates))
	for _, c := range candidates {
		if c == "" {
			continue
		}
		if _, ok := seen[c]; ok {
			continue
		}
		seen[c] = struct{}{}
		out = append(out, c)
	}
	return out
}

func extractUniqueIDs(items []any, path string) ([]string, bool) {
	ids := make([]string, 0, len(items))
	seen := map[string]struct{}{}
	for _, item := range items {
		id, ok := identifierByPath(item, path)
		if !ok || id == "" {
			return nil, false
		}
		if _, exists := seen[id]; exists {
			return nil, false
		}
		seen[id] = struct{}{}
		ids = append(ids, id)
	}
	return ids, true
}

func identifierByPath(v any, path string) (string, bool) {
	cur := v
	for _, seg := range strings.Split(path, ".") {
		m, ok := cur.(map[string]any)
		if !ok {
			return "", false
		}
		next, ok := m[seg]
		if !ok {
			return "", false
		}
		cur = next
	}
	switch x := cur.(type) {
	case string:
		if x == "" {
			return "", false
		}
		return x, true
	case int, int64, float64, bool:
		return fmt.Sprintf("%v", x), true
	default:
		return "", false
	}
}

func multisetEqual(left, right []any, opts Options) bool {
	if len(left) != len(right) {
		return false
	}
	counts := map[string]int{}
	for _, v := range left {
		k := stableListItemKey(v, opts)
		counts[k]++
	}
	for _, v := range right {
		k := stableListItemKey(v, opts)
		counts[k]--
	}
	for _, n := range counts {
		if n != 0 {
			return false
		}
	}
	return true
}

func stableListItemKey(v any, opts Options) string {
	v = normalizeYAMLValue(v)
	if opts.IgnoreWhitespaceChange {
		v = normalizeWhitespaceStrings(v)
	}
	b, err := json.Marshal(v)
	if err == nil {
		return string(b)
	}
	return fmt.Sprintf("%#v", v)
}

func normalizeWhitespaceStrings(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			out[k] = normalizeWhitespaceStrings(vv)
		}
		return out
	case []any:
		out := make([]any, len(x))
		for i := range x {
			out[i] = normalizeWhitespaceStrings(x[i])
		}
		return out
	case string:
		return strings.TrimSpace(x)
	default:
		return x
	}
}

func escapePathSegment(s string) string {
	s = strings.ReplaceAll(s, "\\", "\\\\")
	s = strings.ReplaceAll(s, "]", "\\]")
	return s
}

func formatChanges(in []change) string {
	var b strings.Builder
	for i, c := range in {
		if i > 0 {
			b.WriteByte('\n')
		}
		switch c.Kind {
		case changeAdded:
			fmt.Fprintf(&b, "+ %s: %s", c.Path, renderValue(c.Right))
		case changeRemoved:
			fmt.Fprintf(&b, "- %s: %s", c.Path, renderValue(c.Left))
		default:
			fmt.Fprintf(&b, "~ %s: %s -> %s", c.Path, renderValue(c.Left), renderValue(c.Right))
		}
	}
	return b.String()
}

func renderValue(v any) string {
	if isComposite(v) {
		return shortYAML(v)
	}
	return shortValue(v)
}

func isComposite(v any) bool {
	if v == nil {
		return false
	}
	k := reflect.TypeOf(v).Kind()
	return k == reflect.Map || k == reflect.Slice || k == reflect.Array
}

func shortYAML(v any) string {
	b, err := safeYAMLMarshal(v)
	if err != nil {
		return shortValue(v)
	}
	s := strings.TrimSpace(string(b))
	s = strings.ReplaceAll(s, "\n", "\\n")
	if len(s) > 200 {
		return s[:200] + "..."
	}
	return s
}

func safeYAMLMarshal(v any) (b []byte, err error) {
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("yaml marshal panic: %v", r)
		}
	}()
	return yaml.Marshal(v)
}

func shortValue(v any) string {
	j, err := json.Marshal(v)
	if err != nil {
		return fmt.Sprintf("%v", v)
	}
	s := string(j)
	if len(s) > 160 {
		return s[:160] + "..."
	}
	return s
}
