package dyffcli

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type ColorMode string

const (
	ColorAuto   ColorMode = "auto"
	ColorAlways ColorMode = "always"
	ColorNever  ColorMode = "never"
)

type Options struct {
	ColorMode  ColorMode
	UseColor   *bool
	FromLabel  string
	ToLabel    string
	ShowHeader bool
}

type Entry struct {
	Kind    string `json:"kind"`
	Path    string `json:"path,omitempty"`
	Body    string `json:"body,omitempty"`
	Left    string `json:"left,omitempty"`
	Right   string `json:"right,omitempty"`
	IsKnown bool   `json:"isKnown"`
}

type Stats struct {
	Added   int `json:"added"`
	Removed int `json:"removed"`
	Changed int `json:"changed"`
	Total   int `json:"total"`
}

type JSONReport struct {
	Equal bool    `json:"equal"`
	Stats Stats   `json:"stats"`
	Items []Entry `json:"items"`
}

func Format(diff string, opts Options) string {
	diff = strings.TrimRight(diff, "\n")
	if diff == "" {
		return ""
	}
	lines := strings.Split(diff, "\n")
	entries := make([]entry, 0, len(lines))
	maxPath := 0
	for _, line := range lines {
		e := parseLine(line)
		entries = append(entries, e)
		if pl := visibleLen(e.Path); pl > maxPath {
			maxPath = pl
		}
	}
	if maxPath > 80 {
		maxPath = 80
	}
	useColor := shouldColor(opts)
	var b strings.Builder
	if opts.ShowHeader {
		from := opts.FromLabel
		to := opts.ToLabel
		if from == "" {
			from = "source"
		}
		if to == "" {
			to = "target"
		}
		h := fmt.Sprintf("dyff %s -> %s", from, to)
		if useColor {
			h = ansiBold + h + ansiReset
		}
		b.WriteString(h)
		b.WriteByte('\n')
	}
	for i, e := range entries {
		if i > 0 {
			b.WriteByte('\n')
		}
		b.WriteString(renderEntry(e, maxPath, useColor))
	}
	return b.String()
}

func LabelsFromPaths(fromPath, toPath string) (string, string) {
	return labelForPath(fromPath), labelForPath(toPath)
}

func ParseEntries(diff string) []Entry {
	diff = strings.TrimRight(diff, "\n")
	if diff == "" {
		return nil
	}
	lines := strings.Split(diff, "\n")
	out := make([]Entry, 0, len(lines))
	for _, line := range lines {
		e := parseLine(line)
		out = append(out, Entry{
			Kind:    string(e.Kind),
			Path:    e.Path,
			Body:    e.Body,
			Left:    e.Left,
			Right:   e.Right,
			IsKnown: e.IsKnown,
		})
	}
	return out
}

func ComputeStats(entries []Entry) Stats {
	var s Stats
	for _, e := range entries {
		switch e.Kind {
		case "+":
			s.Added++
		case "-":
			s.Removed++
		case "~":
			s.Changed++
		}
	}
	s.Total = s.Added + s.Removed + s.Changed
	return s
}

func JSON(diff string) ([]byte, error) {
	items := ParseEntries(diff)
	rep := JSONReport{
		Equal: strings.TrimSpace(diff) == "",
		Stats: ComputeStats(items),
		Items: items,
	}
	return json.MarshalIndent(rep, "", "  ")
}

type entry struct {
	Kind    byte
	Path    string
	Body    string
	Left    string
	Right   string
	IsKnown bool
}

func parseLine(line string) entry {
	if len(line) < 3 || line[1] != ' ' {
		return entry{Body: line}
	}
	kind := line[0]
	switch kind {
	case '+', '-':
		rest := line[2:]
		path, body, ok := cutTwo(rest, ": ")
		if !ok {
			return entry{Kind: kind, Body: line[2:], IsKnown: true}
		}
		return entry{Kind: kind, Path: path, Body: body, IsKnown: true}
	case '~':
		rest := line[2:]
		path, payload, ok := cutTwo(rest, ": ")
		if !ok {
			return entry{Kind: kind, Body: line[2:], IsKnown: true}
		}
		left, right, ok := cutTwo(payload, " -> ")
		if !ok {
			return entry{Kind: kind, Path: path, Body: payload, IsKnown: true}
		}
		return entry{Kind: kind, Path: path, Left: left, Right: right, IsKnown: true}
	default:
		return entry{Body: line}
	}
}

func renderEntry(e entry, pathWidth int, color bool) string {
	if !e.IsKnown {
		return e.Body
	}
	prefix := string(e.Kind)
	if color {
		switch e.Kind {
		case '+':
			prefix = ansiGreen + "+" + ansiReset
		case '-':
			prefix = ansiRed + "-" + ansiReset
		case '~':
			prefix = ansiYellow + "~" + ansiReset
		}
	}
	path := e.Path
	if color {
		path = ansiCyan + e.Path + ansiReset
	}
	path = path + strings.Repeat(" ", intMax(1, pathWidth-visibleLen(e.Path)+1))
	switch e.Kind {
	case '+':
		body := e.Body
		if color {
			body = colorizeMultiline(body, ansiGreen)
		}
		return prefix + " " + path + body
	case '-':
		body := e.Body
		if color {
			body = colorizeMultiline(body, ansiRed)
		}
		return prefix + " " + path + body
	case '~':
		left := e.Left
		right := e.Right
		arrow := " -> "
		if color {
			left = colorizeMultiline(left, ansiRed)
			right = colorizeMultiline(right, ansiGreen)
			arrow = " " + ansiDim + "->" + ansiReset + " "
		}
		return prefix + " " + path + left + arrow + right
	default:
		return prefix + " " + path + e.Body
	}
}

func colorizeMultiline(s, colorCode string) string {
	if !strings.Contains(s, "\\n") {
		return colorCode + s + ansiReset
	}
	parts := strings.Split(s, "\\n")
	for i := range parts {
		parts[i] = colorCode + parts[i] + ansiReset
	}
	return strings.Join(parts, "\\n"+ansiDim+"  "+ansiReset)
}

func cutTwo(s, sep string) (string, string, bool) {
	i := strings.Index(s, sep)
	if i < 0 {
		return "", "", false
	}
	return s[:i], s[i+len(sep):], true
}

func visibleLen(s string) int { return len(s) }

func shouldColor(opts Options) bool {
	if opts.UseColor != nil {
		return *opts.UseColor
	}
	switch opts.ColorMode {
	case ColorAlways:
		return true
	case ColorNever:
		return false
	default:
		fi, err := os.Stdout.Stat()
		if err != nil {
			return false
		}
		return (fi.Mode() & os.ModeCharDevice) != 0
	}
}

func labelForPath(path string) string {
	if path == "-" {
		return "stdin"
	}
	if path == "" {
		return "?"
	}
	return filepath.Base(path)
}

func intMax(a, b int) int {
	if a > b {
		return a
	}
	return b
}

const (
	ansiReset  = "\x1b[0m"
	ansiBold   = "\x1b[1m"
	ansiDim    = "\x1b[2m"
	ansiRed    = "\x1b[31m"
	ansiGreen  = "\x1b[32m"
	ansiYellow = "\x1b[33m"
	ansiCyan   = "\x1b[36m"
)
