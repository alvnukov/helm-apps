package verify

import (
	"encoding/json"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/zol/helm-apps/cmd/happ/internal/dyfflike"
)

type Result struct {
	Equal   bool
	Summary string
}

type ResourceComparison struct {
	Key       string `json:"key"`
	Status    string `json:"status"` // equal | changed | missing_in_generated | extra_in_generated
	DiffPath  string `json:"diffPath,omitempty"`
	SourceVal string `json:"sourceVal,omitempty"`
	GenVal    string `json:"genVal,omitempty"`
}

type DetailedResult struct {
	Equal     bool                 `json:"equal"`
	Summary   string               `json:"summary"`
	Resources []ResourceComparison `json:"resources"`
}

func Equivalent(sourceDocs, generatedDocs []map[string]any) Result {
	srcIdx := indexDocs(normalizeDocs(sourceDocs))
	genIdx := indexDocs(normalizeDocs(generatedDocs))

	srcKeys := sortedMapKeys(srcIdx)
	genKeys := sortedMapKeys(genIdx)
	if len(srcKeys) != len(genKeys) {
		return Result{Equal: false, Summary: fmt.Sprintf("resource count differs: source=%d generated=%d", len(srcKeys), len(genKeys))}
	}
	for _, k := range srcKeys {
		if _, ok := genIdx[k]; !ok {
			return Result{Equal: false, Summary: fmt.Sprintf("missing resource in generated chart: %s", k)}
		}
	}
	for _, k := range genKeys {
		if _, ok := srcIdx[k]; !ok {
			return Result{Equal: false, Summary: fmt.Sprintf("extra resource in generated chart: %s", k)}
		}
	}
	for _, k := range srcKeys {
		if path, av, bv, ok := firstDiffPath(srcIdx[k], genIdx[k], ""); ok {
			return Result{Equal: false, Summary: fmt.Sprintf("resource content mismatch: %s at %s (source=%s generated=%s)", k, path, shortJSON(av), shortJSON(bv))}
		}
	}
	return Result{Equal: true, Summary: fmt.Sprintf("equivalent resources: %d", len(srcKeys))}
}

func CompareDetailed(sourceDocs, generatedDocs []map[string]any) DetailedResult {
	srcIdx := indexDocs(normalizeDocs(sourceDocs))
	genIdx := indexDocs(normalizeDocs(generatedDocs))
	srcKeys := sortedMapKeys(srcIdx)
	genKeys := sortedMapKeys(genIdx)

	comp := make([]ResourceComparison, 0, maxInt(len(srcKeys), len(genKeys)))
	allGood := len(srcKeys) == len(genKeys)
	for _, k := range srcKeys {
		if _, ok := genIdx[k]; !ok {
			comp = append(comp, ResourceComparison{Key: k, Status: "missing_in_generated"})
			allGood = false
		}
	}
	for _, k := range genKeys {
		if _, ok := srcIdx[k]; !ok {
			comp = append(comp, ResourceComparison{Key: k, Status: "extra_in_generated"})
			allGood = false
		}
	}
	for _, k := range srcKeys {
		g, ok := genIdx[k]
		if !ok {
			continue
		}
		if path, av, bv, ok := firstDiffPath(srcIdx[k], g, ""); ok {
			comp = append(comp, ResourceComparison{
				Key:       k,
				Status:    "changed",
				DiffPath:  path,
				SourceVal: shortJSON(av),
				GenVal:    shortJSON(bv),
			})
			allGood = false
			continue
		}
		comp = append(comp, ResourceComparison{Key: k, Status: "equal"})
	}
	sort.Slice(comp, func(i, j int) bool {
		if comp[i].Status != comp[j].Status {
			return comp[i].Status < comp[j].Status
		}
		return comp[i].Key < comp[j].Key
	})
	if allGood {
		return DetailedResult{Equal: true, Summary: fmt.Sprintf("equivalent resources: %d", len(srcKeys)), Resources: comp}
	}
	return DetailedResult{
		Equal:     false,
		Summary:   fmt.Sprintf("resource comparison: source=%d generated=%d mismatches=%d", len(srcKeys), len(genKeys), countNonEqual(comp)),
		Resources: comp,
	}
}

func NormalizeDocsForCompare(in []map[string]any) []map[string]any {
	return normalizeDocs(in)
}

func countNonEqual(in []ResourceComparison) int {
	n := 0
	for _, r := range in {
		if r.Status != "equal" {
			n++
		}
	}
	return n
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func firstDiffPath(a, b any, path string) (string, any, any, bool) {
	switch ax := a.(type) {
	case map[string]any:
		bx, ok := b.(map[string]any)
		if !ok {
			return pathOrRoot(path), a, b, true
		}
		keysSet := map[string]struct{}{}
		for k := range ax {
			keysSet[k] = struct{}{}
		}
		for k := range bx {
			keysSet[k] = struct{}{}
		}
		keys := make([]string, 0, len(keysSet))
		for k := range keysSet {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		for _, k := range keys {
			av, aok := ax[k]
			bv, bok := bx[k]
			if !aok || !bok {
				return joinPath(path, k), av, bv, true
			}
			if p, da, db, ok := firstDiffPath(av, bv, joinPath(path, k)); ok {
				return p, da, db, true
			}
		}
		return "", nil, nil, false
	case []any:
		bx, ok := b.([]any)
		if !ok {
			return pathOrRoot(path), a, b, true
		}
		if len(ax) != len(bx) {
			return pathOrRoot(path) + ".length", len(ax), len(bx), true
		}
		for i := range ax {
			if p, da, db, ok := firstDiffPath(ax[i], bx[i], fmt.Sprintf("%s[%d]", pathOrRoot(path), i)); ok {
				return p, da, db, true
			}
		}
		return "", nil, nil, false
	default:
		if !valuesEqual(a, b) {
			return pathOrRoot(path), a, b, true
		}
		return "", nil, nil, false
	}
}

func valuesEqual(a, b any) bool {
	aj, aerr := canonicalJSON(a)
	bj, berr := canonicalJSON(b)
	if aerr == nil && berr == nil {
		return aj == bj
	}
	return fmt.Sprintf("%v", a) == fmt.Sprintf("%v", b)
}

func joinPath(base, key string) string {
	if base == "" {
		return key
	}
	return base + "." + key
}

func pathOrRoot(path string) string {
	if path == "" {
		return "$"
	}
	return path
}

func shortJSON(v any) string {
	b, err := json.Marshal(v)
	if err != nil {
		return fmt.Sprintf("%v", v)
	}
	s := string(b)
	const limit = 120
	if len(s) > limit {
		return strconv.QuoteToASCII(s[:limit] + "...")
	}
	return s
}

func indexDocs(docs []map[string]any) map[string]map[string]any {
	out := make(map[string]map[string]any, len(docs))
	for _, d := range docs {
		key := docKey(d)
		out[key] = d
	}
	return out
}

func docKey(d map[string]any) string {
	apiVersion, _ := d["apiVersion"].(string)
	kind, _ := d["kind"].(string)
	meta, _ := d["metadata"].(map[string]any)
	name, _ := meta["name"].(string)
	ns, _ := meta["namespace"].(string)
	if ns == "" {
		// Helm-rendered manifests often omit namespace for default namespace resources.
		// Treat empty namespace and "default" as equivalent in compare/equivalence keys.
		ns = "default"
	}
	return strings.Join([]string{apiVersion, kind, ns, name}, "/")
}

func normalizeDocs(in []map[string]any) []map[string]any {
	out := make([]map[string]any, 0, len(in))
	for _, d := range in {
		n, ok := normalizeAny(d).(map[string]any)
		if ok {
			dropKnownLibraryCompareNoise(n)
			out = append(out, n)
		}
	}
	return out
}

func dropKnownLibraryCompareNoise(d map[string]any) {
	// helm-apps currently renders pod template metadata.name for workloads.
	// It is library-specific and does not affect workload semantics.
	deleteNestedKey(d, "spec", "template", "metadata", "name")
	deleteNestedKey(d, "spec", "jobTemplate", "spec", "template", "metadata", "name")
}

func deleteNestedKey(root map[string]any, path ...string) {
	if len(path) == 0 || root == nil {
		return
	}
	cur := root
	for i := 0; i < len(path)-1; i++ {
		next, _ := cur[path[i]].(map[string]any)
		if next == nil {
			return
		}
		cur = next
	}
	delete(cur, path[len(path)-1])
}

func normalizeAny(v any) any {
	switch x := v.(type) {
	case map[string]any:
		out := make(map[string]any, len(x))
		for k, vv := range x {
			if k == "status" {
				continue
			}
			if strings.HasPrefix(k, "__") {
				// Ignore importer/library internal bookkeeping fields if they leaked into docs.
				continue
			}
			if k == "metadata" {
				if m, ok := vv.(map[string]any); ok {
					nm := normalizeMetadata(m)
					if nm != nil {
						out[k] = nm
					}
					continue
				}
			}
			nv := normalizeAny(vv)
			if nv == nil {
				continue
			}
			out[k] = nv
		}
		return out
	case []any:
		arr := make([]any, len(x))
		for i := range x {
			arr[i] = normalizeAny(x[i])
		}
		if sorted, ok := sortSemanticList(arr); ok {
			return sorted
		}
		return arr
	default:
		return x
	}
}

func normalizeMetadata(m map[string]any) map[string]any {
	out := make(map[string]any, len(m))
	for k, v := range m {
		if k == "labels" || k == "annotations" {
			continue
		}
		if k == "namespace" {
			if ns, ok := v.(string); ok && (ns == "" || ns == "default") {
				continue
			}
		}
		nv := normalizeAny(v)
		if nv == nil {
			continue
		}
		out[k] = nv
	}
	return out
}

func canonicalJSON(v any) (string, error) {
	b, err := json.Marshal(sortRec(v))
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func sortSemanticList(in []any) ([]any, bool) {
	if len(in) < 2 {
		return in, false
	}
	type item struct {
		v   any
		key string
	}
	items := make([]item, 0, len(in))
	seen := map[string]struct{}{}
	for _, v := range in {
		k, ok := semanticListItemKey(v)
		if !ok {
			return nil, false
		}
		if _, dup := seen[k]; dup {
			return nil, false
		}
		seen[k] = struct{}{}
		items = append(items, item{v: v, key: k})
	}
	sort.Slice(items, func(i, j int) bool { return items[i].key < items[j].key })
	out := make([]any, 0, len(items))
	for _, it := range items {
		out = append(out, it.v)
	}
	return out, true
}

func semanticListItemKey(v any) (string, bool) {
	m, ok := v.(map[string]any)
	if !ok || len(m) == 0 {
		return "", false
	}
	if s, ok := strField(m, "name"); ok {
		// Covers containers, env items, volumes, many k8s lists.
		if mp, ok := strField(m, "mountPath"); ok {
			return "name+mountPath:" + s + "|" + mp, true
		}
		if p, ok := intLikeField(m, "containerPort"); ok {
			return "name+containerPort:" + s + "|" + p, true
		}
		if p, ok := intLikeField(m, "port"); ok {
			return "name+port:" + s + "|" + p, true
		}
		return "name:" + s, true
	}
	if s, ok := nestedStrField(m, "configMapRef", "name"); ok {
		return "configMapRef:" + s, true
	}
	if s, ok := nestedStrField(m, "secretRef", "name"); ok {
		return "secretRef:" + s, true
	}
	if s, ok := strField(m, "mountPath"); ok {
		n, _ := strField(m, "name")
		return "mountPath:" + s + "|name:" + n, true
	}
	if p, ok := intLikeField(m, "containerPort"); ok {
		name, _ := strField(m, "name")
		proto, _ := strField(m, "protocol")
		return "containerPort:" + p + "|name:" + name + "|proto:" + proto, true
	}
	if p, ok := intLikeField(m, "port"); ok {
		name, _ := strField(m, "name")
		tp, _ := strField(m, "targetPort")
		return "port:" + p + "|name:" + name + "|targetPort:" + tp, true
	}
	if h, ok := strField(m, "host"); ok {
		return "host:" + h, true
	}
	if p, ok := strField(m, "path"); ok {
		pt, _ := strField(m, "pathType")
		return "path:" + p + "|pathType:" + pt, true
	}
	if md, _ := m["metadata"].(map[string]any); md != nil {
		if n, ok := strField(md, "name"); ok {
			return "metadata.name:" + n, true
		}
	}
	return "", false
}

func strField(m map[string]any, key string) (string, bool) {
	s, _ := m[key].(string)
	if strings.TrimSpace(s) == "" {
		return "", false
	}
	return s, true
}

func nestedStrField(m map[string]any, key1, key2 string) (string, bool) {
	n, _ := m[key1].(map[string]any)
	if n == nil {
		return "", false
	}
	return strField(n, key2)
}

func intLikeField(m map[string]any, key string) (string, bool) {
	v, ok := m[key]
	if !ok || v == nil {
		return "", false
	}
	switch x := v.(type) {
	case int:
		return strconv.Itoa(x), true
	case int64:
		return fmt.Sprintf("%d", x), true
	case float64:
		if x == float64(int64(x)) {
			return fmt.Sprintf("%d", int64(x)), true
		}
		return fmt.Sprintf("%v", x), true
	case string:
		if strings.TrimSpace(x) == "" {
			return "", false
		}
		return x, true
	default:
		return fmt.Sprintf("%v", x), true
	}
}

func sortRec(v any) any {
	switch x := v.(type) {
	case map[string]any:
		keys := sortedMapKeysAny(x)
		out := make(map[string]any, len(x))
		for _, k := range keys {
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

func sortedMapKeys(m map[string]map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func sortedMapKeysAny(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func DyffAvailable() bool {
	return true
}

func DyffBetweenDocs(from, to map[string]any) (string, error) {
	return dyfflike.BetweenDocs(from, to)
}

func DyffBetweenYAML(fromYAML, toYAML []byte) (string, error) {
	return dyfflike.BetweenYAML(fromYAML, toYAML)
}
