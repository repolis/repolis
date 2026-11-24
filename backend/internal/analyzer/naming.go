package analyzer

import (
	"fmt"
	"path/filepath"
	"slices"
	"sort"
	"strings"
)

func (c Cluster) DistinctDirs(common []string) []string {
	out := make([]string, 0, len(c.TopDirs))
	for _, d := range c.TopDirs {
		parts := strings.Split(d, "/")
		kept := parts[:0]
		for _, p := range parts {
			if !slices.Contains(common, p) {
				kept = append(kept, p)
			}
		}
		if len(kept) > 0 {
			out = append(out, strings.Join(kept, "/"))
		} else {
			out = append(out, d)
		}
	}
	return out
}

func SplitIdentifier(name string) []string {
	var words []string
	var cur []rune
	flush := func() {
		if len(cur) > 0 {
			words = append(words, strings.ToLower(string(cur)))
			cur = cur[:0]
		}
	}
	runes := []rune(name)
	for i, r := range runes {
		switch {
		case r == '_' || r == '-' || r == ' ':
			flush()
		case r >= 'A' && r <= 'Z':
			// Only at a lower->upper transition, so acronyms stay together.
			if i > 0 && runes[i-1] >= 'a' && runes[i-1] <= 'z' {
				flush()
			}
			cur = append(cur, r)
		default:
			cur = append(cur, r)
		}
	}
	flush()
	return words
}

// Measured per cluster, not as a global prefix: libgit2's test suite dilutes
// "git_" below any global threshold, yet "git" is in every district and swamped
// every name the model produced.
func CommonSymbolTokens(clusters []Cluster) []string {
	if len(clusters) < 3 {
		return nil
	}
	counts := map[string]int{}
	for _, c := range clusters {
		seen := map[string]bool{}
		for _, n := range c.TopNames {
			for _, w := range SplitIdentifier(n) {
				if len(w) < 2 || seen[w] {
					continue
				}
				seen[w] = true
				counts[w]++
			}
		}
	}
	threshold := max(len(clusters)*3/5, 2)
	var out []string
	for w, n := range counts {
		if n >= threshold {
			out = append(out, w)
		}
	}
	sort.Strings(out)
	return out
}

func CommonDirSegments(clusters []Cluster) []string {
	counts := map[string]int{}
	for _, c := range clusters {
		seen := map[string]bool{}
		for _, d := range c.TopDirs {
			for _, p := range strings.Split(d, "/") {
				if !seen[p] {
					seen[p] = true
					counts[p]++
				}
			}
		}
	}
	threshold := (len(clusters) * 2) / 3
	if threshold < 2 {
		return nil
	}
	var out []string
	for p, n := range counts {
		if n >= threshold {
			out = append(out, p)
		}
	}
	sort.Strings(out)
	return out
}

// This name shows before the model answers, without a model, and after a vague
// answer, so it has to be good. The dominant directory locates the code, the
// largest symbol names it.
func (c Cluster) FallbackName() string {
	banned := make(map[string]bool, len(c.Noise))
	for _, w := range c.Noise {
		banned[w] = true
	}
	clean := func(s string) string {
		s = strings.TrimSuffix(strings.TrimSuffix(s, ".c"), ".h")
		kept := make([]string, 0, 4)
		for _, w := range SplitIdentifier(s) {
			if !banned[w] || len(kept) > 0 {
				kept = append(kept, w)
			}
		}
		if len(kept) == 0 {
			return ""
		}
		return titleize(strings.Join(kept, " "))
	}

	dir := ""
	for _, d := range c.TopDirs {
		if d == "root" {
			continue
		}
		if v := clean(filepath.Base(d)); v != "" {
			dir = v
			break
		}
	}
	sym := ""
	for _, n := range c.TopNames {
		if v := clean(n); v != "" {
			sym = v
			break
		}
	}

	// The directory reads as a place name on its own; pairing it with an
	// unrelated symbol produced labels like "Cli / Str".
	switch {
	case dir != "":
		return dir
	case sym != "":
		return sym
	default:
		return "District"
	}
}

func titleize(s string) string {
	s = strings.ReplaceAll(strings.ReplaceAll(s, "_", " "), "-", " ")
	parts := strings.Fields(s)
	for i, p := range parts {
		r := []rune(p)
		if len(r) > 0 {
			r[0] = []rune(strings.ToUpper(string(r[0])))[0]
			parts[i] = string(r)
		}
	}
	if len(parts) > 3 {
		parts = parts[:3]
	}
	return strings.Join(parts, " ")
}

func (c Cluster) FallbackTypology() string {
	joined := ""
	for _, d := range c.TopDirs {
		joined += " " + d
	}
	for i, n := range c.TopNames {
		if i > 5 {
			break
		}
		joined += " " + n
	}
	return GuessTypology(joined)
}

func (c Cluster) FallbackSummary() string {
	if len(c.TopDirs) > 0 {
		return fmt.Sprintf("%d entities, mostly in %s", len(c.Buildings), c.TopDirs[0])
	}
	return fmt.Sprintf("%d related code entities", len(c.Buildings))
}
