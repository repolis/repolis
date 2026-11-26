package analyzer

import (
	"os"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
	sitter "github.com/tree-sitter/go-tree-sitter"
)

// For /api/explain only: nothing in the main pipeline sends source to a model.
func ExtractSymbolSource(fullPath string, symbolNames []string) string {
	l := lang.ForFile(fullPath)
	if l == nil {
		return ""
	}
	content, err := os.ReadFile(fullPath)
	if err != nil || len(content) > 4*1024*1024 {
		return ""
	}

	parser := sitter.NewParser()
	defer parser.Close()
	if err := parser.SetLanguage(l.Grammar()); err != nil {
		return ""
	}
	tree := parser.Parse(content, nil)
	if tree == nil {
		return ""
	}
	defer tree.Close()

	targets := make(map[string]bool, len(symbolNames))
	for _, n := range symbolNames {
		if n = strings.TrimSpace(n); n != "" {
			targets[n] = true
		}
	}

	facts := l.Parse(tree.RootNode(), content)

	const budget = 6000
	var out strings.Builder
	quote := func(name string, start, end uint) {
		if !targets[name] || out.Len() > budget || end <= start || int(end) > len(content) {
			return
		}
		out.WriteString(condense(string(content[start:end])))
		out.WriteString("\n\n")
	}

	for _, t := range facts.Types {
		quote(t.Name, t.Start, t.End)
	}
	for _, f := range facts.Funcs {
		quote(f.Name, f.Start, f.End)
	}

	res := strings.ReplaceAll(out.String(), "\t", "  ")
	if len(res) > budget {
		res = res[:budget] + "\n...[truncated]"
	}
	return strings.TrimSpace(res)
}

func condense(s string) string {
	const max = 1200
	if len(s) <= max {
		return s
	}
	return s[:800] + "\n    /* ... */\n" + s[len(s)-200:]
}
