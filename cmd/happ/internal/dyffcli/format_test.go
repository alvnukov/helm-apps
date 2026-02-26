package dyffcli

import (
	"strings"
	"testing"
)

func TestFormat_NoColorReadableColumns(t *testing.T) {
	in := "~ $.a: 1 -> 2\n+ $.long.path: {\"x\":1}\n- $.b: true\n"
	out := Format(in, Options{ColorMode: ColorNever, ShowHeader: true, FromLabel: "a.yaml", ToLabel: "b.yaml"})
	if !strings.Contains(out, "dyff a.yaml -> b.yaml") {
		t.Fatalf("expected header, got:\n%s", out)
	}
	if strings.Contains(out, "\x1b[") {
		t.Fatalf("did not expect ansi color in no-color mode: %q", out)
	}
	if !strings.Contains(out, "~ $.a") || !strings.Contains(out, "+ $.long.path") || !strings.Contains(out, "- $.b") {
		t.Fatalf("expected diff entries, got:\n%s", out)
	}
}

func TestFormat_AlwaysColorAddsANSI(t *testing.T) {
	in := "~ $.a: 1 -> 2"
	out := Format(in, Options{ColorMode: ColorAlways})
	if !strings.Contains(out, "\x1b[") {
		t.Fatalf("expected ansi escapes, got %q", out)
	}
}

func TestFormat_UnknownLinePassthrough(t *testing.T) {
	in := "weird line"
	out := Format(in, Options{ColorMode: ColorNever})
	if out != in {
		t.Fatalf("expected passthrough for unknown line, got %q", out)
	}
}

func TestHelpers_LabelsAndParsing(t *testing.T) {
	from, to := LabelsFromPaths("/tmp/a.yaml", "-")
	if from != "a.yaml" || to != "stdin" {
		t.Fatalf("unexpected labels: %q %q", from, to)
	}
	e := parseLine("~ $.x: old -> new")
	if !e.IsKnown || e.Kind != '~' || e.Path != "$.x" || e.Left != "old" || e.Right != "new" {
		t.Fatalf("unexpected parsed line: %#v", e)
	}
	if a, b, ok := cutTwo("a::b", "::"); !ok || a != "a" || b != "b" {
		t.Fatalf("unexpected cutTwo result: %q %q %v", a, b, ok)
	}
}

func TestParseEntriesStatsAndJSON(t *testing.T) {
	entries := ParseEntries("~ $.a: 1 -> 2\n+ $.b: 3\n- $.c: 4\n")
	if len(entries) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(entries))
	}
	stats := ComputeStats(entries)
	if stats.Added != 1 || stats.Removed != 1 || stats.Changed != 1 || stats.Total != 3 {
		t.Fatalf("unexpected stats: %#v", stats)
	}
	j, err := JSON("~ $.a: 1 -> 2\n")
	if err != nil {
		t.Fatalf("JSON error: %v", err)
	}
	s := string(j)
	if !strings.Contains(s, "\"equal\": false") || !strings.Contains(s, "\"changed\": 1") {
		t.Fatalf("unexpected json report: %s", s)
	}
}
