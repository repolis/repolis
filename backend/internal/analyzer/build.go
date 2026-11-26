package analyzer

import (
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"time"

	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/models"
)

// Renderable on its own with no LLM, which is what lets the API stream a usable
// city in seconds.
type Draft struct {
	Buildings []models.Building
	Edges     []models.DependencyEdge
	Clusters  []Cluster
	Cycles    []models.Cycle
	Symbols   *SymbolTable
	Stats     models.CityStats

	firstCommit time.Time
	lastCommit  time.Time
}

// The symbol table is passed in so the pipeline can build twice: with rule-only
// associations, then after the LLM has settled the ambiguous ones.
func BuildDraft(raw *RawExtraction, history map[string]*git.FileHistory, st *SymbolTable) *Draft {
	if st == nil {
		st = BuildSymbolTable(raw)
	}

	ownerOf := make(map[string]string, len(raw.Functions))
	for _, a := range st.Assocs {
		if a.Chosen != "" {
			ownerOf[funcKey(a.Fn)] = a.Chosen
		} else {
			ownerOf[funcKey(a.Fn)] = moduleID(a.Fn.SourceFile)
		}
	}

	d := &Draft{Symbols: st}
	d.assemble(raw, st, ownerOf)
	d.Edges = BuildDependencies(raw, st, ownerOf)
	d.attachHistory(history)
	d.computeCoupling()

	cycles := FindCycles(d.Buildings, d.Edges)
	MarkCycles(d.Buildings, d.Edges, cycles)
	d.Cycles = cycles

	d.Clusters = ClusterBuildings(d.Buildings, d.Edges)
	for i := range d.Clusters {
		d.Clusters[i].SymbolPrefix = st.SymbolPrefix()
	}
	// Words shared by most districts identify none of them, so neither the
	// fallback names nor the prompts use them.
	noise := append(CommonSymbolTokens(d.Clusters), CommonDirSegments(d.Clusters)...)
	for i := range d.Clusters {
		d.Clusters[i].Noise = noise
		d.Clusters[i].Language = raw.DominantLanguage()
	}
	d.computeStats(raw, st)
	return d
}

type ownerTotals struct {
	methods       []string
	source        string
	maxComplexity int
	sumComplexity int
}

func (d *Draft) assemble(raw *RawExtraction, st *SymbolTable, ownerOf map[string]string) {
	owners := make(map[string]*ownerTotals)
	for _, a := range st.Assocs {
		owner := ownerOf[funcKey(a.Fn)]
		o := owners[owner]
		if o == nil {
			o = &ownerTotals{}
			owners[owner] = o
		}
		o.methods = append(o.methods, a.Fn.Name)
		o.sumComplexity += a.Fn.Complexity
		o.maxComplexity = max(o.maxComplexity, a.Fn.Complexity)
		// A building's attribution is the weakest of its members'.
		switch {
		case a.Source == SourceLLM:
			o.source = SourceLLM
		case a.Source == SourceRule || a.Source == SourceSyntax:
			if o.source == "" {
				o.source = a.Source
			}
		}
	}
	totalsOf := func(id string) ownerTotals {
		if o := owners[id]; o != nil {
			return *o
		}
		return ownerTotals{}
	}

	filesByPath := make(map[string]*FileInfo, len(raw.Files))
	for i := range raw.Files {
		filesByPath[raw.Files[i].Path] = &raw.Files[i]
	}
	fileOf := func(path string) FileInfo {
		if f := filesByPath[path]; f != nil {
			return *f
		}
		return FileInfo{}
	}

	for _, t := range st.Types {
		o := totalsOf(t.ID)
		sort.Strings(o.methods)
		src := o.source
		if src == "" {
			src = "none"
		}
		d.Buildings = append(d.Buildings, models.Building{
			ID: t.ID, Name: t.Name, Kind: "type",
			SourceFile: t.SourceFile, Dir: t.Dir, Namespace: t.Namespace,
			NumFields: len(t.Fields), NumMethods: len(o.methods),
			Fields: nonNil(t.Fields), Methods: nonNil(o.methods),
			LinesOfCode: t.LOC, AssocSource: src,
			Language:      fileOf(t.SourceFile).Language,
			MaxComplexity: o.maxComplexity, SumComplexity: o.sumComplexity,
		})
	}

	// One "module" building per file with unattached functions left. Without
	// these, free functions - most of a C codebase - are invisible, and
	// libcsv renders as three buildings.
	moduleFuncs := make(map[string][]string)
	for _, a := range st.Assocs {
		if a.Chosen == "" {
			moduleFuncs[a.Fn.SourceFile] = append(moduleFuncs[a.Fn.SourceFile], a.Fn.Name)
		}
	}
	files := make([]string, 0, len(moduleFuncs))
	for f := range moduleFuncs {
		files = append(files, f)
	}
	sort.Strings(files)

	for _, f := range files {
		fns := moduleFuncs[f]
		sort.Strings(fns)
		file := fileOf(f)
		o := totalsOf(moduleID(f))
		d.Buildings = append(d.Buildings, models.Building{
			ID: moduleID(f), Name: filepath.Base(f), Kind: "module",
			SourceFile: f, Dir: dirOf(f), Namespace: file.Namespace,
			NumFields: file.FileScopeVars, NumMethods: len(fns),
			Fields: []string{}, Methods: fns,
			LinesOfCode: file.LinesOfCode, AssocSource: SourceRule,
			Language:      file.Language,
			MaxComplexity: o.maxComplexity, SumComplexity: o.sumComplexity,
		})
	}

	slices.SortFunc(d.Buildings, func(a, b models.Building) int { return strings.Compare(a.ID, b.ID) })
}

func nonNil(s []string) []string {
	if s == nil {
		return []string{}
	}
	return s
}

// Churn becomes a percentile so the renderer gets a bounded signal whatever the
// repo's pace.
func (d *Draft) attachHistory(history map[string]*git.FileHistory) {
	now := time.Now()
	churns := make([]int, 0, len(d.Buildings))

	// The repo's own epoch: ages are days since it, so the timeline is a
	// plain number line and the renderer handles no dates.
	var first, last time.Time
	for _, h := range history {
		if h.FirstSeen.IsZero() {
			continue
		}
		if first.IsZero() || h.FirstSeen.Before(first) {
			first = h.FirstSeen
		}
		if h.LastModified.After(last) {
			last = h.LastModified
		}
	}
	d.firstCommit, d.lastCommit = first, last

	for i := range d.Buildings {
		b := &d.Buildings[i]
		h := history[b.SourceFile]
		if h == nil {
			continue
		}
		b.CommitChurn = h.Churn
		b.PrimaryAuthor = h.PrimaryAuthor
		if !h.FirstSeen.IsZero() {
			b.FirstSeen = h.FirstSeen.Format(time.RFC3339)
			if !first.IsZero() {
				b.BornDay = daysBetween(first, h.FirstSeen)
			}
		}
		if !h.LastModified.IsZero() {
			b.LastModified = h.LastModified.Format(time.RFC3339)
			b.AgeDays = max(0, daysBetween(h.LastModified, now))
		}
		churns = append(churns, h.Churn)
	}

	if len(churns) == 0 {
		return
	}
	sort.Ints(churns)
	for i := range d.Buildings {
		b := &d.Buildings[i]
		if b.CommitChurn <= 0 {
			continue
		}
		idx := sort.SearchInts(churns, b.CommitChurn)
		b.ChurnRank = float64(idx) / float64(len(churns))
	}
}

func daysBetween(from, to time.Time) int {
	return int(to.Sub(from).Hours() / 24)
}

func (d *Draft) computeStats(raw *RawExtraction, st *SymbolTable) {
	s := models.CityStats{
		TotalBuildings: len(d.Buildings),
		TotalFiles:     len(raw.Files),
		Languages:      raw.Languages,
		SkippedDirs:    raw.SkippedDirs,
		Cycles:         d.Cycles,
	}
	if s.SkippedDirs == nil {
		s.SkippedDirs = []string{}
	}
	for _, b := range d.Buildings {
		if b.Kind == "module" {
			s.TotalModules++
		} else {
			s.TotalTypes++
		}
		s.TotalMethods += b.NumMethods
	}
	for _, f := range raw.Files {
		s.TotalLOC += f.LinesOfCode
	}
	for _, a := range st.Assocs {
		switch a.Source {
		case SourceSyntax:
			s.MethodsBySyntax++
		case SourceRule:
			s.MethodsByRule++
		case SourceLLM:
			s.MethodsByLLM++
		default:
			s.Orphans++
		}
	}
	if !d.firstCommit.IsZero() {
		s.FirstCommit = d.firstCommit.Format(time.RFC3339)
		s.LastCommit = d.lastCommit.Format(time.RFC3339)
		s.HistoryDays = daysBetween(d.firstCommit, d.lastCommit)
	}
	d.Stats = s
}

// Deliberately no "dead code" flag: zero fan-in usually means a library's
// public API, and that judgement is the user's.
func (d *Draft) computeCoupling() {
	in := make(map[string]int, len(d.Buildings))
	out := make(map[string]int, len(d.Buildings))
	for _, e := range d.Edges {
		out[e.Source]++
		in[e.Target]++
	}

	fanIns := make([]int, 0, len(d.Buildings))
	for i := range d.Buildings {
		b := &d.Buildings[i]
		b.FanIn = in[b.ID]
		b.FanOut = out[b.ID]
		if total := b.FanIn + b.FanOut; total > 0 {
			b.Instability = float64(b.FanOut) / float64(total)
		}
		fanIns = append(fanIns, b.FanIn)
	}

	// Relative to this repo: "heavily depended upon" differs wildly between
	// a library and an application.
	sort.Ints(fanIns)
	cutoff := 3
	if n := len(fanIns); n > 0 {
		cutoff = max(3, fanIns[n*95/100])
	}
	for i := range d.Buildings {
		d.Buildings[i].Hub = d.Buildings[i].FanIn >= cutoff
	}
}
