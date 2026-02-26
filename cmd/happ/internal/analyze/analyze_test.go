package analyze

import (
	"os"
	"path/filepath"
	"testing"
)

func TestChart_FindsValuesPathsAndOccurrences(t *testing.T) {
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	chartPath := filepath.Clean(filepath.Join(wd, "..", "..", "testdata", "simple-chart"))
	rep, err := Chart(chartPath)
	if err != nil {
		t.Fatalf("Chart() error: %v", err)
	}
	if rep.TemplateFiles == 0 {
		t.Fatalf("expected template files > 0")
	}
	if rep.Summary.UniqueValuesPaths == 0 {
		t.Fatalf("expected values paths > 0")
	}
	assertContains(t, rep.ValuesPaths, "Values.config.mode")
	assertContains(t, rep.ValuesPaths, "Values.service.port")
	if len(rep.Occurrences) == 0 {
		t.Fatalf("expected occurrences > 0")
	}
}

func assertContains(t *testing.T, arr []string, want string) {
	t.Helper()
	for _, s := range arr {
		if s == want {
			return
		}
	}
	t.Fatalf("expected %q in %#v", want, arr)
}
