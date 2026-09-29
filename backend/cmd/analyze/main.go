// Command analyze runs the deterministic extraction pipeline against a local
// checkout and prints a diagnostic report. No LLM, no server, no network —
// this is the fast loop for working on extraction quality.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"sort"
	"time"

	"github.com/joho/godotenv"
	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/llm"
	"github.com/repolis/repolis/backend/internal/logger"
)

func runRefine(path string, raw *analyzer.RawExtraction, history map[string]*git.FileHistory) {
	_ = godotenv.Load()
	logger.SetLevel(logger.InfoLevel)

	client, err := llm.NewClient(nil)
	if err != nil {
		fmt.Fprintln(os.Stderr, "llm:", err)
		return
	}

	ctx := context.Background()
	st := analyzer.BuildSymbolTable(raw)

	t := time.Now()
	applied := client.AdjudicateAssociations(ctx, st, nil)
	tAssoc := time.Since(t)

	draft := analyzer.BuildDraft(raw, history, st)

	t = time.Now()
	labels := client.LabelClusters(ctx, draft.Clusters, nil)
	tLabel := time.Since(t)

	fmt.Printf("\n== LLM refinement ==\n")
	fmt.Printf("calls       %d (%d cached)\n", client.Calls(), client.CacheHits())
	fmt.Printf("timing      adjudicate %6.0fms (%d applied) | label %6.0fms\n",
		ms(tAssoc), applied, ms(tLabel))
	fmt.Println("districts:")
	for i, l := range labels {
		fmt.Printf("  %2d. %-30s [%-9s] %3d buildings  %s\n",
			i+1, trunc(l.Name, 30), l.Typology, len(draft.Clusters[i].Buildings), trunc(l.Summary, 50))
	}
}

func main() {
	out := flag.String("out", "", "write the draft city map as JSON to this path")
	useLLM := flag.Bool("llm", false, "also run the LLM refinement pass")
	quiet := flag.Bool("quiet", true, "suppress per-file logging")
	flag.Parse()

	if flag.NArg() < 1 {
		fmt.Fprintln(os.Stderr, "usage: analyze [-out city.json] <repo-path>")
		os.Exit(2)
	}
	path := flag.Arg(0)
	if *quiet {
		logger.SetLevel(logger.WarnLevel)
	}

	t0 := time.Now()
	raw, err := analyzer.ExtractRepository(path)
	if err != nil {
		fmt.Fprintln(os.Stderr, "extract:", err)
		os.Exit(1)
	}
	tExtract := time.Since(t0)

	t1 := time.Now()
	history, herr := git.ExtractHistory(path)
	if herr != nil {
		fmt.Fprintln(os.Stderr, "history warning:", herr)
	}
	tHistory := time.Since(t1)

	t2 := time.Now()
	draft := analyzer.BuildDraft(raw, history, nil)
	tBuild := time.Since(t2)

	s := draft.Stats
	fmt.Printf("== %s ==\n", path)
	fmt.Printf("timing      parse %6.0fms | history %6.0fms | assemble %6.0fms | total %6.0fms\n",
		ms(tExtract), ms(tHistory), ms(tBuild), ms(time.Since(t0)))
	fmt.Printf("files       %d (%d LOC), %d dirs skipped, languages %v\n",
		s.TotalFiles, s.TotalLOC, len(s.SkippedDirs), raw.Languages)
	fmt.Printf("buildings   %d  (%d types, %d modules)\n", s.TotalBuildings, s.TotalTypes, s.TotalModules)
	fmt.Printf("methods     %d attributed | by syntax %d | by rule %d | residual-for-llm %d | orphan-module %d\n",
		s.TotalMethods, s.MethodsBySyntax, s.MethodsByRule, len(draft.Symbols.Residual()), s.Orphans)
	fmt.Printf("edges       %d\n", len(draft.Edges))
	hubs, isolated, maxCx, sumCx := 0, 0, 0, 0
	for _, b := range draft.Buildings {
		if b.Hub {
			hubs++
		}
		if b.FanIn == 0 && b.FanOut == 0 {
			isolated++
		}
		if b.MaxComplexity > maxCx {
			maxCx = b.MaxComplexity
		}
		sumCx += b.SumComplexity
	}
	fmt.Printf("coupling    %d hubs | %d isolated | worst complexity %d | total %d\n",
		hubs, isolated, maxCx, sumCx)
	fmt.Printf("clusters    %d\n", len(draft.Clusters))
	findings, tangles, within := 0, 0, 0
	sizes := ""
	for _, c := range draft.Cycles {
		switch {
		case !c.CrossModule():
			within++
		case c.Reliable:
			findings++
			if findings <= 8 {
				sizes += fmt.Sprintf(" %d", c.Size)
			}
		default:
			tangles++
		}
	}
	fmt.Printf("cycles      %d cross-module findings (sizes:%s) | %d large tangles | %d within one module\n",
		findings, sizes, tangles, within)

	zeroField, zeroMethod, loc1 := 0, 0, 0
	withHistory := 0
	for _, b := range draft.Buildings {
		if b.NumFields == 0 {
			zeroField++
		}
		if b.NumMethods == 0 {
			zeroMethod++
		}
		if b.LinesOfCode <= 1 {
			loc1++
		}
		if b.CommitChurn > 0 {
			withHistory++
		}
	}
	pct := func(n int) float64 {
		if s.TotalBuildings == 0 {
			return 0
		}
		return 100 * float64(n) / float64(s.TotalBuildings)
	}
	fmt.Printf("quality     zero-field %d (%.0f%%) | zero-method %d (%.0f%%) | LOC<=1 %d (%.0f%%) | with git history %d (%.0f%%)\n",
		zeroField, pct(zeroField), zeroMethod, pct(zeroMethod), loc1, pct(loc1), withHistory, pct(withHistory))

	fmt.Println("\nclusters:")
	for i, c := range draft.Clusters {
		fmt.Printf("  %2d. %-28s %3d buildings  dirs=%v\n", i+1,
			trunc(c.FallbackName()+" ["+c.FallbackTypology()+"]", 28), len(c.Buildings), c.TopDirs)
	}

	fmt.Println("\ntop buildings by methods:")
	top := append([]struct {
		n string
		m int
		f int
		k string
	}{}, nil...)
	for _, b := range draft.Buildings {
		top = append(top, struct {
			n string
			m int
			f int
			k string
		}{b.Name, b.NumMethods, b.NumFields, b.Kind})
	}
	sort.Slice(top, func(i, j int) bool { return top[i].m > top[j].m })
	for i := 0; i < len(top) && i < 10; i++ {
		fmt.Printf("  %-32s %s  methods=%-4d fields=%d\n", trunc(top[i].n, 32), top[i].k, top[i].m, top[i].f)
	}

	if *useLLM {
		runRefine(path, raw, history)
	}

	if *out != "" {
		b, _ := json.MarshalIndent(draft.Buildings, "", " ")
		_ = os.WriteFile(*out, b, 0o644)
		fmt.Printf("\nwrote %s\n", *out)
	}
}

func ms(d time.Duration) float64 { return float64(d.Microseconds()) / 1000.0 }

func trunc(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n-1] + "…"
}
