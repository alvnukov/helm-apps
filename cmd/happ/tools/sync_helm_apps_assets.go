package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
)

func main() {
	wd, err := os.Getwd()
	if err != nil {
		fatal(err)
	}
	// Expected cwd when run via `go generate ./assets`: .../cmd/happ/assets
	src := filepath.Clean(filepath.Join(wd, "../../../charts/helm-apps"))
	dst := filepath.Clean(filepath.Join(wd, "helm-apps"))

	if err := syncDir(src, dst); err != nil {
		fatal(err)
	}
	fmt.Fprintf(os.Stdout, "synced embedded helm-apps asset: %s -> %s\n", src, dst)
}

func syncDir(src, dst string) error {
	info, err := os.Stat(src)
	if err != nil {
		return fmt.Errorf("stat src: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("source is not a directory: %s", src)
	}
	if err := os.MkdirAll(dst, 0o755); err != nil {
		return err
	}

	seen := map[string]struct{}{}
	err = filepath.Walk(src, func(path string, info fs.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		seen[rel] = struct{}{}
		target := filepath.Join(dst, rel)
		if info.IsDir() {
			return os.MkdirAll(target, 0o755)
		}
		b, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return err
		}
		return os.WriteFile(target, b, info.Mode().Perm())
	})
	if err != nil {
		return err
	}

	// Prune files/dirs that no longer exist in source (keep dst root).
	return pruneExtra(dst, seen)
}

func pruneExtra(dst string, seen map[string]struct{}) error {
	var paths []string
	if err := filepath.Walk(dst, func(path string, info fs.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(dst, path)
		if err != nil {
			return err
		}
		if rel == "." {
			return nil
		}
		paths = append(paths, rel)
		return nil
	}); err != nil {
		return err
	}
	// Remove deepest first.
	sort.Slice(paths, func(i, j int) bool { return len(paths[i]) > len(paths[j]) })
	for _, rel := range paths {
		if _, ok := seen[rel]; ok {
			continue
		}
		target := filepath.Join(dst, rel)
		if err := os.Remove(target); err != nil && !os.IsNotExist(err) {
			// Dir may require RemoveAll if non-empty due to traversal order mismatch; use RemoveAll as fallback.
			if err := os.RemoveAll(target); err != nil && !os.IsNotExist(err) {
				return err
			}
		}
	}
	return nil
}

func fatal(err error) {
	fmt.Fprintln(os.Stderr, "sync_helm_apps_assets:", err)
	os.Exit(1)
}
