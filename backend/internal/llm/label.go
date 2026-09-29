package llm

import (
	"context"
	"fmt"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/logger"
)

// Label is the naming the LLM contributes for one district.
type Label struct {
	Name     string
	Typology string
	Summary  string
	Tags     []string
	FromLLM  bool
}

// The prompt deliberately says nothing about cities. Told it was naming
// neighbourhoods, qwen2.5:1.5b took the metaphor literally and produced
// "CityBuildingSimulator - Simulates a city's development" for a maths
// library. Describing the actual task - naming a group of C symbols - fixes it.
//
// TYPE is also deliberately absent: asked to choose a typology, the model
// answered "core" for every district regardless of content. Typology is
// derived instead from the name it produces plus the folder paths, both of
// which are checkable. The model is asked only for the thing it is good at.
var labelSystemFmt = `You give a short technical name to a group of related %s source symbols.
Judge only from the symbol names and folder paths given.

Name the SUBSYSTEM, not the project. Never include the library or program name,
and never use the words "library", "subsystem", "system" or "module" on their own -
they describe every group and so distinguish none.

Reply with exactly two lines, nothing else:
NAME: <2-4 words naming what this code does>
DESC: <at most 10 words, what this code does>

Example output:
NAME: Object Database
DESC: Reads and writes packed object storage`

// exampleName and exampleDesc are the worked example in the system prompt. An
// unsure model sometimes copies it verbatim, which produced a ripgrep district
// called "Regex Matcher" described as "Reads and writes packed object
// storage". Echoed text is not an answer.
const (
	exampleName = "object database"
	exampleDesc = "reads and writes packed object storage"
)

// LabelClusters names each district with one small, independent call.
//
// One call per cluster rather than one call for all of them: each prompt stays
// ~150 tokens, well inside the effective context of a small local model; each
// is cached and retried independently; and a failure costs one district's name
// instead of the whole city's. Cluster *membership* was already decided
// deterministically, so the LLM cannot damage the structure.
func (c *Client) LabelClusters(ctx context.Context, clusters []analyzer.Cluster, progress func(done, total int)) []Label {
	labels := make([]Label, len(clusters))
	for i, cl := range clusters {
		labels[i] = Label{
			Name:     cl.FallbackName(),
			Typology: cl.FallbackTypology(),
			Summary:  cl.FallbackSummary(),
			Tags:     fallbackTags(cl),
		}
	}
	if len(clusters) == 0 {
		return labels
	}

	language := clusters[0].Language
	if language == "" {
		language = "source"
	}
	logger.Log(logger.InfoLevel, "Labelling %d %s districts (1 call each)", len(clusters), language)

	// Strip what every cluster has in common. On libgit2 the model saw
	// "git_repository, git_index, git_odb..." in all sixteen prompts and
	// named all sixteen districts "Git <something>"; the shared prefix and
	// the shared path segments are pure noise for a naming task.
	common := analyzer.CommonDirSegments(clusters)
	// Words shared by most districts: the project's own name, and anything
	// else that is true of the whole codebase.
	noise := append([]string(nil), clusters[0].Noise...)
	if p := strings.Trim(clusters[0].SymbolPrefix, "_0123456789"); p != "" {
		noise = append(noise, strings.ToLower(p))
	}

	var done int
	out := runBatch(ctx, c.concurrency, len(clusters), func(i int) *Label {
		cl := clusters[i]

		var b strings.Builder
		fmt.Fprintf(&b, "Symbols: %s\n", strings.Join(stripNoise(cl.TopNames, noise), ", "))
		if dirs := cl.DistinctDirs(common); len(dirs) > 0 {
			fmt.Fprintf(&b, "Folders: %s\n", strings.Join(dirs, ", "))
		}
		fmt.Fprintf(&b, "Size: %d entities\n", len(cl.Buildings))
		if len(noise) > 0 {
			fmt.Fprintf(&b, "Banned words (true of every group here): %s\n",
				strings.Join(noise, ", "))
		}

		resp, err := c.complete(ctx, request{
			kind:      "label.v3",
			model:     c.fastModel,
			system:    fmt.Sprintf(labelSystemFmt, language),
			user:      b.String(),
			maxTokens: 48,
			stop:      []string{"\n\n"},
		})
		if progress != nil {
			done++
			progress(done, len(clusters))
		}
		if err != nil {
			logger.Log(logger.WarnLevel, "District %d labelling failed, keeping fallback name: %v", i+1, err)
			return nil
		}
		return parseLabel(stripThinking(resp))
	})

	named := 0
	for i, l := range out {
		if l == nil {
			continue
		}
		if strings.EqualFold(strings.TrimSpace(l.Summary), exampleDesc) {
			l.Summary = ""
		}
		if strings.EqualFold(strings.TrimSpace(l.Name), exampleName) {
			l.Name = ""
		}
		if l.Name != "" {
			trimmed := stripProjectToken(l.Name, noise)
			if !isVague(trimmed, noise) {
				labels[i].Name = trimmed
				named++
			}
			// The produced name is often the strongest typology signal
			// available ("Graphics Engine" -> core, "UI Elements" ->
			// interface), and unlike the model's own TYPE answer it is
			// classified by code we control.
			if t := analyzer.GuessTypology(l.Name); t != "unknown" {
				labels[i].Typology = t
			}
		}
		if l.Summary != "" {
			labels[i].Summary = l.Summary
			if labels[i].Typology == "unknown" {
				if t := analyzer.GuessTypology(l.Summary); t != "unknown" {
					labels[i].Typology = t
				}
			}
		}
		labels[i].FromLLM = true
	}

	// District names must be unique: they are the primary way a user
	// identifies a place in the city, and duplicates make labels useless.
	seen := make(map[string]int)
	for i := range labels {
		base := labels[i].Name
		candidates := []string{base}
		if dirs := clusters[i].DistinctDirs(common); len(dirs) > 0 && dirs[0] != "root" {
			candidates = append(candidates, base+" ("+dirs[0]+")")
		}
		if len(clusters[i].TopNames) > 0 {
			sym := strings.TrimLeft(strings.TrimPrefix(clusters[i].TopNames[0], clusters[i].SymbolPrefix), "_")
			candidates = append(candidates, base+" ("+sym+")")
		}
		for n := 2; n < 2+len(clusters); n++ {
			candidates = append(candidates, fmt.Sprintf("%s %d", base, n))
		}
		for _, cand := range candidates {
			key := strings.ToLower(cand)
			if seen[key] == 0 {
				labels[i].Name = cand
				seen[key] = 1
				break
			}
		}
	}

	logger.Log(logger.InfoLevel, "Districts named: %d by model, %d by fallback", named, len(clusters)-named)
	return labels
}

// vagueNames are labels that describe every cluster equally and so identify
// none. A rejected name falls back to the cluster's dominant directory, which
// at least locates it.
var vagueNames = map[string]bool{
	"library": true, "subsystem": true, "system": true, "module": true,
	"core": true, "utilities": true, "utility": true, "misc": true,
	"components": true, "code": true, "implementation": true,
}

// isVague reports whether a name, once the project's own token is removed,
// says nothing that distinguishes this district from any other.
func isVague(name string, noise []string) bool {
	banned := make(map[string]bool, len(noise))
	for _, w := range noise {
		banned[w] = true
	}
	kept := 0
	for _, w := range strings.Fields(strings.ToLower(name)) {
		w = strings.Trim(w, ".,()")
		if w == "" || vagueNames[w] || banned[w] {
			continue
		}
		kept++
	}
	return kept == 0
}

// stripProjectToken removes the project's own name from a district label.
// Every district of libgit2 is "Git something"; the word carries no
// information inside that city and crowds out the two words that do.
func stripProjectToken(name string, noise []string) string {
	if len(noise) == 0 {
		return name
	}
	banned := make(map[string]bool, len(noise))
	for _, w := range noise {
		banned[w] = true
	}
	words := strings.Fields(name)
	kept := words[:0]
	for _, w := range words {
		if banned[strings.ToLower(strings.Trim(w, ".,()"))] {
			continue
		}
		kept = append(kept, w)
	}
	if len(kept) == 0 {
		return name
	}
	return strings.Join(kept, " ")
}

func parseLabel(s string) *Label {
	l := &Label{}
	for _, line := range strings.Split(s, "\n") {
		// Local models routinely decorate the keys with markdown: a reply of
		// "**NAME:** `Hash Functions`" is well-formed as far as they are
		// concerned, and was silently dropped by a plain prefix match.
		line = strings.TrimLeft(strings.TrimSpace(line), "*_#-> \t")
		upper := strings.ToUpper(line)
		switch {
		case strings.HasPrefix(upper, "NAME:"):
			l.Name = cleanLabel(line[5:], 42)
		case strings.HasPrefix(upper, "DESC:"):
			l.Summary = cleanLabel(line[5:], 90)
		}
	}
	if l.Name == "" && l.Typology == "" && l.Summary == "" {
		return nil
	}
	return l
}

// stripNoise removes codebase-wide words from the symbol names shown to the
// model, leaving only the parts that distinguish this group.
func stripNoise(names, noise []string) []string {
	if len(noise) == 0 {
		return names
	}
	banned := make(map[string]bool, len(noise))
	for _, w := range noise {
		banned[w] = true
	}
	out := make([]string, 0, len(names))
	for _, n := range names {
		kept := make([]string, 0, 4)
		for _, w := range analyzer.SplitIdentifier(n) {
			if !banned[w] {
				kept = append(kept, w)
			}
		}
		if len(kept) == 0 {
			continue
		}
		out = append(out, strings.Join(kept, "_"))
	}
	if len(out) == 0 {
		return names
	}
	return out
}

func cleanLabel(s string, max int) string {
	s = strings.TrimSpace(s)
	// Strip repeatedly: "** `Name` **" needs more than one pass.
	for i := 0; i < 4; i++ {
		trimmed := strings.TrimSpace(strings.Trim(s, "\"'*`_#:"))
		if trimmed == s {
			break
		}
		s = trimmed
	}
	if i := strings.IndexAny(s, "\n\r"); i != -1 {
		s = s[:i]
	}
	if len(s) > max {
		s = strings.TrimSpace(s[:max])
	}
	return s
}

func fallbackTags(cl analyzer.Cluster) []string {
	tags := make([]string, 0, 3)
	seen := make(map[string]bool)
	for _, d := range cl.TopDirs {
		if d == "root" || seen[d] {
			continue
		}
		seen[d] = true
		parts := strings.Split(d, "/")
		tags = append(tags, parts[len(parts)-1])
		if len(tags) >= 3 {
			break
		}
	}
	return tags
}
