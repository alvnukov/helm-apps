package dyfflike

import (
	"strings"
	"testing"
)

func TestBetweenValues_EqualReturnsEmpty(t *testing.T) {
	out, err := BetweenValues(
		map[string]any{"a": 1, "b": map[string]any{"c": "x"}},
		map[string]any{"a": 1, "b": map[string]any{"c": "x"}},
	)
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff, got %q", out)
	}
}

func TestBetweenDocs_Wrapper(t *testing.T) {
	out, err := BetweenDocs(map[string]any{"a": 1}, map[string]any{"a": 2})
	if err != nil {
		t.Fatalf("BetweenDocs error: %v", err)
	}
	if !strings.Contains(out, "~ $.a: 1 -> 2") {
		t.Fatalf("unexpected BetweenDocs output: %q", out)
	}
}

func TestBetweenValues_ChangeAddRemoveAndListDiffs(t *testing.T) {
	out, err := BetweenValues(
		map[string]any{
			"a": 1,
			"b": map[string]any{"x": "old", "remove": true},
			"l": []any{1, 2},
		},
		map[string]any{
			"a": 2,
			"b": map[string]any{"x": "new", "add": "y"},
			"l": []any{1, 3, 4},
		},
	)
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	for _, want := range []string{
		"~ $.a: 1 -> 2",
		"~ $.b.x: \"old\" -> \"new\"",
		"- $.b.remove: true",
		"+ $.b.add: \"y\"",
		"~ $.l.length: 2 -> 3",
		"~ $.l[1]: 2 -> 3",
		"+ $.l[2]: 4",
	} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected diff to contain %q, got:\n%s", want, out)
		}
	}
}

func TestBetweenValues_TypeMismatchAndMarshalFallback(t *testing.T) {
	out, err := BetweenValues(map[string]any{"a": map[string]any{"x": 1}}, map[string]any{"a": []any{1}})
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	if !strings.Contains(out, "~ $.a:") {
		t.Fatalf("expected type mismatch diff, got %q", out)
	}

	// Non-JSON-marshalable values force scalarEqual/shortValue fallback formatting branches.
	out, err = BetweenValues(map[string]any{"a": make(chan int)}, map[string]any{"a": make(chan int)})
	if err != nil {
		t.Fatalf("BetweenValues fallback error: %v", err)
	}
	if strings.TrimSpace(out) == "" {
		// channels with different addresses should differ; if same behavior changes later, this still exercises fallback path.
		t.Fatalf("expected non-empty diff for distinct channels")
	}
}

func TestBetweenYAML_SingleAndMultiDoc(t *testing.T) {
	out, err := BetweenYAML([]byte("a: 1\n"), []byte("a: 2\n"))
	if err != nil {
		t.Fatalf("BetweenYAML single-doc error: %v", err)
	}
	if !strings.Contains(out, "~ $.a: 1 -> 2") {
		t.Fatalf("unexpected single-doc diff: %q", out)
	}

	out, err = BetweenYAML([]byte("a: 1\n---\nb: 2\n"), []byte("a: 1\n---\nb: 3\n"))
	if err != nil {
		t.Fatalf("BetweenYAML multi-doc error: %v", err)
	}
	if !strings.Contains(out, "~ $[1].b: 2 -> 3") {
		t.Fatalf("unexpected multi-doc diff: %q", out)
	}
}

func TestBetweenYAML_InvalidYAML(t *testing.T) {
	if _, err := BetweenYAML([]byte("a: [\n"), []byte("a: 1\n")); err == nil {
		t.Fatalf("expected parse error for invalid source yaml")
	}
	if _, err := BetweenYAML([]byte("a: 1\n"), []byte("a: [\n")); err == nil {
		t.Fatalf("expected parse error for invalid generated yaml")
	}
}

func TestBetweenYAML_EmptyDocuments(t *testing.T) {
	out, err := BetweenYAML([]byte("---\n...\n"), []byte(""))
	if err != nil {
		t.Fatalf("BetweenYAML empty docs error: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff for nil-vs-nil docs, got %q", out)
	}
}

func TestNormalizeYAMLValue_MapAnyAndShortValue(t *testing.T) {
	v := normalizeYAMLValue(map[any]any{1: map[any]any{"x": "y"}})
	m, ok := v.(map[string]any)
	if !ok {
		t.Fatalf("expected normalized map[string]any, got %#v", v)
	}
	inner := m["1"].(map[string]any)
	if inner["x"] != "y" {
		t.Fatalf("unexpected normalized nested map: %#v", m)
	}
	long := shortValue(strings.Repeat("a", 300))
	if !strings.HasSuffix(long, "...") {
		t.Fatalf("expected shortened value, got %q", long)
	}
	if got := shortValue(make(chan int)); !strings.Contains(got, "0x") {
		t.Fatalf("expected fmt fallback for non-json value, got %q", got)
	}
}

func TestBetweenValuesWithOptions_IgnoreWhitespaceChange(t *testing.T) {
	out, err := BetweenValuesWithOptions(
		map[string]any{"a": " hello "},
		map[string]any{"a": "hello"},
		Options{IgnoreWhitespaceChange: true},
	)
	if err != nil {
		t.Fatalf("BetweenValuesWithOptions error: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff when ignoring whitespace, got %q", out)
	}
}

func TestBetweenValues_DefaultIgnoreOrderForScalarList(t *testing.T) {
	out, err := BetweenValues(
		map[string]any{"env": []any{"A", "B", "C"}},
		map[string]any{"env": []any{"C", "A", "B"}},
	)
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff for list reorder, got %q", out)
	}
}

func TestBetweenValues_IdentifierBasedListMatchingByName(t *testing.T) {
	out, err := BetweenValues(
		map[string]any{"items": []any{
			map[string]any{"name": "b", "value": 2},
			map[string]any{"name": "a", "value": 1},
		}},
		map[string]any{"items": []any{
			map[string]any{"name": "a", "value": 10},
			map[string]any{"name": "b", "value": 2},
		}},
	)
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	if !strings.Contains(out, "~ $.items[name=a].value: 1 -> 10") {
		t.Fatalf("expected identifier-based diff path, got:\n%s", out)
	}
	if strings.Contains(out, ".length") {
		t.Fatalf("unexpected length diff for same-size named list: %s", out)
	}
}

func TestBetweenValuesWithOptions_AdditionalIdentifier(t *testing.T) {
	out, err := BetweenValuesWithOptions(
		map[string]any{"items": []any{
			map[string]any{"meta": map[string]any{"id": "x"}, "value": 1},
			map[string]any{"meta": map[string]any{"id": "y"}, "value": 2},
		}},
		map[string]any{"items": []any{
			map[string]any{"meta": map[string]any{"id": "y"}, "value": 20},
			map[string]any{"meta": map[string]any{"id": "x"}, "value": 1},
		}},
		Options{
			IgnoreOrderChanges:    true,
			AdditionalIdentifiers: []string{"meta.id"},
		},
	)
	if err != nil {
		t.Fatalf("BetweenValuesWithOptions error: %v", err)
	}
	if !strings.Contains(out, "~ $.items[meta.id=y].value: 2 -> 20") {
		t.Fatalf("expected meta.id identifier diff, got:\n%s", out)
	}
}

func TestBetweenValuesWithOptions_NonUniqueIdentifiersFallBack(t *testing.T) {
	out, err := BetweenValuesWithOptions(
		map[string]any{"items": []any{
			map[string]any{"name": "dup", "value": 1},
			map[string]any{"name": "dup", "value": 2},
		}},
		map[string]any{"items": []any{
			map[string]any{"name": "dup", "value": 2},
			map[string]any{"name": "dup", "value": 1},
		}},
		Options{IgnoreOrderChanges: true},
	)
	if err != nil {
		t.Fatalf("BetweenValuesWithOptions error: %v", err)
	}
	// Same multiset, duplicate identifiers: diff should still be empty (multiset fallback).
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff via multiset fallback, got:\n%s", out)
	}
}

func TestBetweenValuesWithOptions_DisableIgnoreOrderShowsReorderNoise(t *testing.T) {
	out, err := BetweenValuesWithOptions(
		[]any{1, 2, 3},
		[]any{3, 2, 1},
		Options{IgnoreOrderChanges: false},
	)
	if err != nil {
		t.Fatalf("BetweenValuesWithOptions error: %v", err)
	}
	if !strings.Contains(out, "~ $[0]: 1 -> 3") {
		t.Fatalf("expected index-based diff when ignore-order disabled, got %q", out)
	}
}

func TestBetweenYAML_MultiDocK8sReorderIgnored(t *testing.T) {
	src := []byte(`
apiVersion: v1
kind: ConfigMap
metadata:
  name: cm-a
---
apiVersion: v1
kind: Service
metadata:
  name: svc-a
`)
	gen := []byte(`
apiVersion: v1
kind: Service
metadata:
  name: svc-a
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: cm-a
`)
	out, err := BetweenYAML(src, gen)
	if err != nil {
		t.Fatalf("BetweenYAML error: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Fatalf("expected empty diff for K8s doc reorder, got:\n%s", out)
	}
}

func TestBetweenYAML_MultiDocSameNameDifferentKind_NoFalseMatch(t *testing.T) {
	src := []byte(`
apiVersion: v1
kind: ConfigMap
metadata:
  name: same
data:
  k: v1
---
apiVersion: v1
kind: Secret
metadata:
  name: same
type: Opaque
`)
	gen := []byte(`
apiVersion: v1
kind: Secret
metadata:
  name: same
type: kubernetes.io/tls
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: same
data:
  k: v1
`)
	out, err := BetweenYAML(src, gen)
	if err != nil {
		t.Fatalf("BetweenYAML error: %v", err)
	}
	if !strings.Contains(out, "~ $[k8s=v1|Secret|default|same].type: \"Opaque\" -> \"kubernetes.io/tls\"") {
		t.Fatalf("expected Secret type diff with K8s composite id, got:\n%s", out)
	}
}

func TestBetweenValues_IdentifierList_AddRemoveAndPathEscaping(t *testing.T) {
	out, err := BetweenValues(
		map[string]any{"items": []any{
			map[string]any{"name": "a]1", "v": 1},
		}},
		map[string]any{"items": []any{
			map[string]any{"name": "b", "v": 2},
		}},
	)
	if err != nil {
		t.Fatalf("BetweenValues error: %v", err)
	}
	if !strings.Contains(out, "- $.items[name=a\\]1]") {
		t.Fatalf("expected escaped identifier in remove path, got:\n%s", out)
	}
	if !strings.Contains(out, "+ $.items[name=b]") {
		t.Fatalf("expected add path for identifier list, got:\n%s", out)
	}
}

func TestHelpers_IdentifierAndCandidateUtilities(t *testing.T) {
	cands := listIdentifierCandidates(Options{AdditionalIdentifiers: []string{"name", "meta.id", "", "meta.id"}})
	if count := strings.Count(strings.Join(cands, ","), "meta.id"); count != 1 {
		t.Fatalf("expected deduped candidates, got %#v", cands)
	}

	if id, ok := identifierByPath(map[string]any{"a": map[string]any{"b": 7}}, "a.b"); !ok || id != "7" {
		t.Fatalf("expected numeric id conversion, got id=%q ok=%v", id, ok)
	}
	if _, ok := identifierByPath(map[string]any{"a": "x"}, "a.b"); ok {
		t.Fatalf("expected identifierByPath miss on non-map traversal")
	}
	if _, ok := identifierByPath(map[string]any{"a": map[string]any{"b": []any{1}}}, "a.b"); ok {
		t.Fatalf("expected identifierByPath unsupported type miss")
	}
}

func TestHelpers_K8sDocIDFailuresAndDefaults(t *testing.T) {
	if _, ok := extractK8sDocIDs([]any{"not-a-map"}); ok {
		t.Fatalf("expected failure for non-map item")
	}
	if _, ok := extractK8sDocIDs([]any{map[string]any{"apiVersion": "v1", "kind": "ConfigMap"}}); ok {
		t.Fatalf("expected failure for missing metadata.name")
	}
	ids, ok := extractK8sDocIDs([]any{map[string]any{
		"apiVersion": "v1",
		"kind":       "ConfigMap",
		"metadata": map[string]any{
			"name": "a",
		},
	}})
	if !ok || len(ids) != 1 || !strings.Contains(ids[0], "|default|a") {
		t.Fatalf("expected default namespace in k8s id, got ids=%#v ok=%v", ids, ok)
	}
}

func TestHelpers_WhitespaceNormalizationAndStableKeyFallback(t *testing.T) {
	n := normalizeWhitespaceStrings(map[string]any{
		"a": "  x ",
		"l": []any{" y ", map[string]any{"z": " z "}},
	}).(map[string]any)
	if n["a"] != "x" {
		t.Fatalf("unexpected normalized string: %#v", n)
	}
	if stable := stableListItemKey(map[string]any{"bad": make(chan int)}, Options{}); stable == "" {
		t.Fatalf("expected non-empty fallback stable key")
	}
	if stable := stableListItemKey([]any{" x "}, Options{IgnoreWhitespaceChange: true}); !strings.Contains(stable, "\"x\"") {
		t.Fatalf("expected trimmed whitespace in stable key, got %q", stable)
	}
}

func TestHelpers_RenderValueCompositeAndFallbacks(t *testing.T) {
	if isComposite(nil) {
		t.Fatalf("nil should not be composite")
	}
	if !isComposite(map[string]any{"a": 1}) || !isComposite([]any{1}) {
		t.Fatalf("map/slice should be composite")
	}
	if got := shortYAML(map[string]any{"bad": make(chan int)}); got == "" {
		t.Fatalf("expected fallback shortYAML output")
	}
	if got := renderValue(map[string]any{"k": "v"}); !strings.Contains(got, "k: v") {
		t.Fatalf("expected yaml-ish render value, got %q", got)
	}
}
