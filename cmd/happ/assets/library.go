package assets

import (
	"embed"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
)

//go:generate go run ../tools/sync_helm_apps_assets.go
//go:embed all:helm-apps/**
var embedded embed.FS

// ExtractHelmAppsChart writes the embedded helm-apps library chart into dst.
// dst should be the final chart directory path (e.g. ".../charts/helm-apps").
func ExtractHelmAppsChart(dst string) error {
	srcRoot := "helm-apps"
	if err := os.MkdirAll(dst, 0o755); err != nil {
		return err
	}
	return fs.WalkDir(embedded, srcRoot, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(srcRoot, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		target := filepath.Join(dst, rel)
		if d.IsDir() {
			return os.MkdirAll(target, 0o755)
		}
		b, err := embedded.ReadFile(path)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return err
		}
		return os.WriteFile(target, b, 0o644)
	})
}

// HasHelmAppsChart returns true when the embedded asset contains a valid chart root.
func HasHelmAppsChart() bool {
	_, err := embedded.ReadFile("helm-apps/Chart.yaml")
	return err == nil
}

func MustHaveHelmAppsChart() {
	if !HasHelmAppsChart() {
		panic(fmt.Sprintf("%s missing from embedded assets", "helm-apps"))
	}
}
