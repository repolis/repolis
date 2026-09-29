package analyzer

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/models"
)

// Draft is the fully deterministic city: everything derivable from the AST,
// the call graph and git history, with no LLM at all. It is renderable on its
// own, which is what lets the API stream a usable city in seconds.
type Draft struct {
	Buildings []models.Building
	Edges     []models.DependencyEdge
	Clusters  []Cluster
	Cycles    []models.Cycle
	Symbols   *SymbolTable
	Stats     models.CityStats

	firstCommit time.Time
	lastCommit  time.Time

	byID map[string]int
}

// BuildDraft assembles the deterministic city. The symbol table is passed in
// so the pipeline can assemble twice from it: once with rule-only
// associations, again after the LLM has settled the ambiguous ones.
func BuildDraft(raw *RawExtraction, history map[string]*git.FileHistory, st *SymbolTable) *Draft {
	if st == nil {
		st = BuildSymbolTable(raw)
	}

	// Owner of every function: its associated type, else its file's module.
	ownerOf := make(map[string]string, len(raw.Functions))
	for _, a := range st.Assocs {
		if a.Chosen != "" {
			ownerOf[funcKey(a.Fn)] = a.Chosen
		} else {
			ownerOf[funcKey(a.Fn)] = moduleID(a.Fn.SourceFile)
		}
	}

	d := &Draft{Symbols: st, byID: make(map[string]int)}
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

func (d *Draft) assemble(raw *RawExtraction, st *SymbolTable, ownerOf map[string]string) {
	methodsOf := make(map[string][]string)
	sourceOf := make(map[string]string)
	maxCx := make(map[string]int)
	sumCx := make(map[string]int)

	for _, a := range st.Assocs {
		owner := ownerOf[funcKey(a.Fn)]
		methodsOf[owner] = append(methodsOf[owner], a.Fn.Name)
		sumCx[owner] += a.Fn.Complexity
		if a.Fn.Complexity > maxCx[owner] {
			maxCx[owner] = a.Fn.Complexity
		}
		// A building's attribution is the weakest of its members'.
		switch {
		case a.Source == SourceLLM:
			if sourceOf[owner] != SourceLLM {
				sourceOf[owner] = SourceLLM
			}
		case a.Source == SourceRule || a.Source == SourceSyntax:
			if sourceOf[owner] == "" {
				sourceOf[owner] = a.Source
			}
		}
	}

	fileLOC := make(map[string]int, len(raw.Files))
	fileVars := make(map[string]int, len(raw.Files))
	fileLang := make(map[string]string, len(raw.Files))
	fileNS := make(map[string]string, len(raw.Files))
	for _, f := range raw.Files {
		fileLOC[f.Path] = f.LinesOfCode
		fileVars[f.Path] = f.FileScopeVars
		fileLang[f.Path] = f.Language
		fileNS[f.Path] = f.Namespace
	}

	// One building per concrete type definition.
	for _, t := range st.Types {
		methods := methodsOf[t.ID]
		sort.Strings(methods)
		src := sourceOf[t.ID]
		if src == "" {
			src = "none"
		}
		d.add(models.Building{
			ID: t.ID, Name: t.Name, Kind: "type",
			SourceFile: t.SourceFile, Dir: t.Dir, Namespace: t.Namespace,
			NumFields: len(t.Fields), NumMethods: len(methods),
			Fields: nonNil(t.Fields), Methods: nonNil(methods),
			LinesOfCode: t.LOC, AssocSource: src,
			Language:      fileLang[t.SourceFile],
			MaxComplexity: maxCx[t.ID], SumComplexity: sumCx[t.ID],
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
		d.add(models.Building{
			ID: moduleID(f), Name: filepath.Base(f), Kind: "module",
			SourceFile: f, Dir: dirOf(f), Namespace: fileNS[f],
			NumFields: fileVars[f], NumMethods: len(fns),
			Fields: []string{}, Methods: fns,
			LinesOfCode: fileLOC[f], AssocSource: SourceRule,
			Language:      fileLang[f],
			MaxComplexity: maxCx[moduleID(f)], SumComplexity: sumCx[moduleID(f)],
		})
	}

	sort.Slice(d.Buildings, func(i, j int) bool { return d.Buildings[i].ID < d.Buildings[j].ID })
	d.byID = make(map[string]int, len(d.Buildings))
	for i := range d.Buildings {
		d.byID[d.Buildings[i].ID] = i
	}
}

func (d *Draft) add(b models.Building) {
	d.Buildings = append(d.Buildings, b)
}

func nonNil(s []string) []string {
	if s == nil {
		return []string{}
	}
	return s
}

// attachHistory copies git metrics onto each building, turning raw churn into
// a percentile so the renderer gets a bounded signal whatever the repo's pace.
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
				b.BornDay = int(h.FirstSeen.Sub(first).Hours() / 24)
			}
		}
		if !h.LastModified.IsZero() {
			b.LastModified = h.LastModified.Format(time.RFC3339)
			days := int(now.Sub(h.LastModified).Hours() / 24)
			if days < 0 {
				days = 0
			}
			b.AgeDays = days
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
		s.HistoryDays = int(d.lastCommit.Sub(d.firstCommit).Hours() / 24)
	}
	d.Stats = s
}

// computeCoupling derives fan-in, fan-out and instability from the finished
// edge set. Deliberately absent: a "dead code" flag - zero fan-in usually
// means a library's public API, not dead code, and that judgement is the
// user's to make from the numbers.
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
	cutoff := 0
	if n := len(fanIns); n > 0 {
		cutoff = fanIns[(n*95)/100]
	}
	if cutoff < 3 {
		cutoff = 3
	}
	for i := range d.Buildings {
		d.Buildings[i].Hub = d.Buildings[i].FanIn >= cutoff
	}
}

// FallbackName derives a district name with no LLM. It has to be good: it is
// the name before the model answers, without a model, and after a vague model
// answer. The dominant directory locates the code, the largest symbol names it.
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

// titleize turns snake_case or a bare path segment into display words.
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

// FallbackTypology guesses a typology from directory and symbol names.
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

// BuildingByID gives the pipeline and /api/explain a lookup.
func (d *Draft) BuildingByID(id string) (models.Building, bool) {
	if i, ok := d.byID[id]; ok {
		return d.Buildings[i], true
	}
	return models.Building{}, false
}
