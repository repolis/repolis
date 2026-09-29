// Package pipeline turns a checkout into a CityMap.
//
// It runs in two publishable passes:
//
//	Pass 1 (deterministic, seconds): AST extraction, rule-based association,
//	        call graph, git history, graph clustering, fallback district names.
//	        This city is complete and navigable on its own.
//	Pass 2 (LLM, tens of seconds): adjudicate the ambiguous associations and
//	        name the districts, then re-assemble.
//
// Publishing pass 1 immediately is what decouples "the city is usable" from
// "the local model has finished". It also means an LLM outage degrades to
// directory-named districts rather than to nothing.
package pipeline

import (
	"context"
	"sort"
	"time"

	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/llm"
	"github.com/repolis/repolis/backend/internal/logger"
	"github.com/repolis/repolis/backend/internal/models"
)

// Stage reports progress to the SSE channel.
type Stage struct {
	Name  string `json:"name"`
	Done  int    `json:"done"`
	Total int    `json:"total"`
}

type Reporter func(Stage)

// State carries everything the two passes share.
type State struct {
	Raw       *analyzer.RawExtraction
	History   map[string]*git.FileHistory
	Symbols   *analyzer.SymbolTable
	ClonePath string
}

// Extract runs the deterministic half and returns both the reusable state and
// a fully renderable city.
func Extract(clonePath string, report Reporter) (*State, *models.CityMap, error) {
	t0 := time.Now()
	report(Stage{Name: "parsing"})

	raw, err := analyzer.ExtractRepository(clonePath)
	if err != nil {
		return nil, nil, err
	}

	report(Stage{Name: "history"})
	history, herr := git.ExtractHistory(clonePath)
	if herr != nil {
		logger.Log(logger.WarnLevel, "git history unavailable, continuing without it: %v", herr)
		history = map[string]*git.FileHistory{}
	}

	report(Stage{Name: "linking"})
	st := analyzer.BuildSymbolTable(raw)
	draft := analyzer.BuildDraft(raw, history, st)
	draft.Stats.ExtractMillis = time.Since(t0).Milliseconds()

	city := compose(draft, fallbackLabels(draft.Clusters), false)
	return &State{Raw: raw, History: history, Symbols: st, ClonePath: clonePath}, city, nil
}

// Refine runs the LLM half and returns the improved city.
func Refine(ctx context.Context, s *State, client *llm.Client, report Reporter) *models.CityMap {
	t0 := time.Now()

	report(Stage{Name: "associating"})
	client.AdjudicateAssociations(ctx, s.Symbols, func(done, total int) {
		report(Stage{Name: "associating", Done: done, Total: total})
	})

	// Re-assemble: adjudication moves functions between buildings, which
	// changes heights, the call graph and therefore the clustering.
	draft := analyzer.BuildDraft(s.Raw, s.History, s.Symbols)

	report(Stage{Name: "naming"})
	labels := client.LabelClusters(ctx, draft.Clusters, func(done, total int) {
		report(Stage{Name: "naming", Done: done, Total: total})
	})

	city := compose(draft, labels, true)
	city.Stats.LLMCalls = int(client.Calls())
	city.Stats.LLMCacheHits = int(client.CacheHits())
	city.Stats.SemanticMillis = time.Since(t0).Milliseconds()
	return city
}

func fallbackLabels(clusters []analyzer.Cluster) []llm.Label {
	out := make([]llm.Label, len(clusters))
	for i, c := range clusters {
		out[i] = llm.Label{
			Name:     c.FallbackName(),
			Typology: c.FallbackTypology(),
			Summary:  c.FallbackSummary(),
			Tags:     []string{},
		}
	}
	// Fallback names come from directory basenames and can collide.
	seen := map[string]int{}
	for i := range out {
		if n := seen[out[i].Name]; n > 0 {
			seen[out[i].Name] = n + 1
			out[i].Name += " " + itoa(n+1)
		} else {
			seen[out[i].Name] = 1
		}
	}
	return out
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var b []byte
	for n > 0 {
		b = append([]byte{byte('0' + n%10)}, b...)
		n /= 10
	}
	return string(b)
}

func compose(d *analyzer.Draft, labels []llm.Label, refined bool) *models.CityMap {
	byID := make(map[string]models.Building, len(d.Buildings))
	for _, b := range d.Buildings {
		byID[b.ID] = b
	}

	districts := make([]models.District, 0, len(d.Clusters))
	assigned := make(map[string]bool, len(d.Buildings))

	for i, cl := range d.Clusters {
		l := llm.Label{Name: cl.FallbackName(), Typology: cl.FallbackTypology(), Summary: cl.FallbackSummary()}
		if i < len(labels) {
			l = labels[i]
		}
		if l.Tags == nil {
			l.Tags = []string{}
		}

		bs := make([]models.Building, 0, len(cl.Buildings))
		for _, id := range cl.Buildings {
			if b, ok := byID[id]; ok {
				bs = append(bs, b)
				assigned[id] = true
			}
		}
		if len(bs) == 0 {
			continue
		}
		sort.Slice(bs, func(a, b int) bool { return bs[a].ID < bs[b].ID })

		districts = append(districts, models.District{
			ID:        "d" + itoa(i),
			Name:      l.Name,
			Typology:  l.Typology,
			Summary:   l.Summary,
			Tags:      l.Tags,
			Buildings: bs,
		})
	}

	// Nothing may be silently dropped between clustering and rendering.
	var leftovers []models.Building
	for _, b := range d.Buildings {
		if !assigned[b.ID] {
			leftovers = append(leftovers, b)
		}
	}
	if len(leftovers) > 0 {
		districts = append(districts, models.District{
			ID: "d-rest", Name: "Unclustered", Typology: "unknown",
			Summary: "Entities with no strong connections",
			Tags:    []string{}, Buildings: leftovers,
		})
	}

	stats := d.Stats
	stats.Refined = refined

	return &models.CityMap{
		Districts:    districts,
		Dependencies: d.Edges,
		Stats:        stats,
	}
}
