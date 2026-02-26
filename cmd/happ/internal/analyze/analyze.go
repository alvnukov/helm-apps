package analyze

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

type Report struct {
	ChartPath       string       `json:"chartPath"`
	TemplateFiles   int          `json:"templateFiles"`
	Occurrences     []Occurrence `json:"occurrences"`
	ValuesPaths     []string     `json:"valuesPaths"`
	RiskyConstructs []RiskyHit   `json:"riskyConstructs"`
	Summary         Summary      `json:"summary"`
}

type Occurrence struct {
	File          string   `json:"file"`
	Line          int      `json:"line"`
	Action        string   `json:"action"`
	ValuesPaths   []string `json:"valuesPaths"`
	PipeFunctions []string `json:"pipeFunctions,omitempty"`
	FormatHint    string   `json:"formatHint,omitempty"`
}

type RiskyHit struct {
	File    string `json:"file"`
	Line    int    `json:"line"`
	Kind    string `json:"kind"`
	Snippet string `json:"snippet"`
}

type Summary struct {
	Occurrences       int            `json:"occurrences"`
	UniqueValuesPaths int            `json:"uniqueValuesPaths"`
	PipeFunctionFreq  map[string]int `json:"pipeFunctionFreq"`
	RiskyCounts       map[string]int `json:"riskyCounts"`
}

var (
	actionRe     = regexp.MustCompile(`\{\{[-]?\s*(.*?)\s*[-]?\}\}`)
	valuesPathRe = regexp.MustCompile(`\.Values((?:\.[A-Za-z0-9_-]+)+)`)
	pipeFuncRe   = regexp.MustCompile(`\|\s*([A-Za-z_][A-Za-z0-9_]*)`)
)

var riskyKinds = []string{"tpl", "lookup", "required", "fail", "now", "randAlpha", "randAlphaNum", "randNumeric", "genCA", "genSelfSignedCert"}

func Chart(chartPath string) (Report, error) {
	tplDir := filepath.Join(chartPath, "templates")
	files := []string{}
	err := filepath.Walk(tplDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			return nil
		}
		files = append(files, path)
		return nil
	})
	if err != nil {
		return Report{}, fmt.Errorf("walk templates: %w", err)
	}
	sort.Strings(files)

	rep := Report{
		ChartPath: chartPath,
		Summary: Summary{
			PipeFunctionFreq: map[string]int{},
			RiskyCounts:      map[string]int{},
		},
	}
	pathSet := map[string]struct{}{}

	for _, f := range files {
		b, err := os.ReadFile(f)
		if err != nil {
			return rep, err
		}
		rep.TemplateFiles++
		lines := strings.Split(string(b), "\n")
		for i, line := range lines {
			lineNo := i + 1
			for _, m := range actionRe.FindAllStringSubmatch(line, -1) {
				action := strings.TrimSpace(m[1])
				if action == "" {
					continue
				}
				valuesMatches := valuesPathRe.FindAllStringSubmatch(action, -1)
				if len(valuesMatches) == 0 {
					collectRiskyHits(&rep, rel(chartPath, f), lineNo, action)
					continue
				}
				paths := make([]string, 0, len(valuesMatches))
				for _, vm := range valuesMatches {
					p := "Values" + vm[1]
					paths = append(paths, p)
					pathSet[p] = struct{}{}
				}
				paths = uniqSorted(paths)
				pipes := []string{}
				for _, pm := range pipeFuncRe.FindAllStringSubmatch(action, -1) {
					fn := pm[1]
					pipes = append(pipes, fn)
					rep.Summary.PipeFunctionFreq[fn]++
				}
				occ := Occurrence{
					File:          rel(chartPath, f),
					Line:          lineNo,
					Action:        action,
					ValuesPaths:   paths,
					PipeFunctions: uniqStable(pipes),
					FormatHint:    guessFormatHint(action, pipes),
				}
				rep.Occurrences = append(rep.Occurrences, occ)
				rep.Summary.Occurrences++
				collectRiskyHits(&rep, rel(chartPath, f), lineNo, action)
			}
		}
	}

	rep.ValuesPaths = sortedSet(pathSet)
	rep.Summary.UniqueValuesPaths = len(rep.ValuesPaths)
	return rep, nil
}

func WriteReport(path string, rep Report) error {
	b, err := json.MarshalIndent(rep, "", "  ")
	if err != nil {
		return err
	}
	if path == "" {
		_, err = os.Stderr.Write(append(b, '\n'))
		return err
	}
	return os.WriteFile(path, append(b, '\n'), 0o644)
}

func collectRiskyHits(rep *Report, file string, line int, action string) {
	for _, k := range riskyKinds {
		if strings.Contains(action, k) {
			rep.RiskyConstructs = append(rep.RiskyConstructs, RiskyHit{
				File: file, Line: line, Kind: k, Snippet: action,
			})
			rep.Summary.RiskyCounts[k]++
		}
	}
}

func guessFormatHint(action string, pipes []string) string {
	if strings.Contains(action, "toYaml") {
		return "yaml-block"
	}
	for _, p := range pipes {
		switch p {
		case "b64enc":
			return "base64-string"
		case "quote", "squote":
			return "quoted-scalar"
		}
	}
	return "scalar-or-template"
}

func rel(chartPath, file string) string {
	if r, err := filepath.Rel(chartPath, file); err == nil {
		return r
	}
	return file
}

func uniqSorted(in []string) []string {
	set := map[string]struct{}{}
	for _, s := range in {
		set[s] = struct{}{}
	}
	return sortedSet(set)
}

func uniqStable(in []string) []string {
	set := map[string]struct{}{}
	out := make([]string, 0, len(in))
	for _, s := range in {
		if _, ok := set[s]; ok {
			continue
		}
		set[s] = struct{}{}
		out = append(out, s)
	}
	return out
}

func sortedSet(set map[string]struct{}) []string {
	out := make([]string, 0, len(set))
	for k := range set {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}
