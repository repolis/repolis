package analyzer

import (
	"context"
	"os"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
	sitter "github.com/smacker/go-tree-sitter"
)

// ExtractSymbolSource returns the source of the named symbols, for /api/explain
// only - nothing in the main pipeline sends source to a model. It works off the
// byte spans each language reports, so it knows nothing about any grammar.
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
	parser.SetLanguage(l.Grammar())
	tree, err := parser.ParseCtx(context.Background(), nil, content)
	if err != nil {
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
	quote := func(name string, start, end uint32) {
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

// condense keeps a definition readable without spending the prompt budget on
// one long body.
func condense(s string) string {
	const max = 1200
	if len(s) <= max {
		return s
	}
	return s[:800] + "\n    /* ... */\n" + s[len(s)-200:]
}
