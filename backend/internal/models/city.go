package models

// Building is one node of the city: a concrete type, or a "module" standing
// in for the free functions of a file.
type Building struct {
	// "<source_file>::<name>". Edges reference buildings by ID, never by Name:
	// C allows two file-scoped structs to share a name.
	ID         string `json:"id"`
	Name       string `json:"name"`
	Kind       string `json:"kind"` // "type" | "module"
	SourceFile string `json:"source_file"`
	Dir        string `json:"dir"`
	Namespace  string `json:"namespace"` // module or package
	Language   string `json:"language"`

	NumFields   int      `json:"num_fields"`
	NumMethods  int      `json:"num_methods"`
	Fields      []string `json:"fields"`
	Methods     []string `json:"methods"`
	LinesOfCode int      `json:"lines_of_code"`

	// Cyclomatic complexity: worst single function, and the total.
	MaxComplexity int `json:"max_complexity"`
	SumComplexity int `json:"sum_complexity"`

	FanIn  int `json:"fan_in"`
	FanOut int `json:"fan_out"`
	// Martin's I = out/(in+out). 0 when the building has no edges.
	Instability float64 `json:"instability"`
	Hub         bool    `json:"hub"` // top few percent by fan-in

	CommitChurn  int     `json:"commit_churn"`
	ChurnRank    float64 `json:"churn_rank"` // 0..1 percentile within the repo
	LastModified string  `json:"last_modified"`
	FirstSeen    string  `json:"first_seen"`
	// Days from the repository's first commit, so the timeline needs no dates.
	BornDay       int    `json:"born_day"`
	AgeDays       int    `json:"age_days"`
	PrimaryAuthor string `json:"primary_author"`

	Summary string `json:"summary"`

	CycleID int `json:"cycle_id"` // 0 when not in a cycle

	// How the methods were attributed: syntax | rule | llm | none. Shown in
	// the inspector so inferred data is never mistaken for fact.
	AssocSource string `json:"assoc_source"`
}

// District is a cluster of the call graph, named by the LLM. Not a directory,
// though directories weight the clustering.
type District struct {
	ID        string     `json:"id"`
	Name      string     `json:"name"`
	Typology  string     `json:"typology"`
	Summary   string     `json:"summary"`
	Tags      []string   `json:"tags"`
	Buildings []Building `json:"buildings"`
}

// DependencyEdge is a call-site-accurate edge. Kind is "call" or "type".
type DependencyEdge struct {
	Source string `json:"source"` // building ID
	Target string `json:"target"` // building ID
	Weight int    `json:"weight"` // number of call sites / references
	Kind   string `json:"kind"`
	// InCycle is true when both ends sit in the same dependency cycle.
	InCycle bool `json:"in_cycle"`
}

// Cycle is a strongly connected component of the call graph with more than one
// member.
type Cycle struct {
	ID      int      `json:"id"`
	Size    int      `json:"size"`
	Members []string `json:"members"`
	// Small enough to trust: the call graph is heuristic, so one wrong edge
	// can merge two large components into a tangle that is not real.
	Reliable bool `json:"reliable"`
	// Modules spanned. A cycle inside one module is normal; one that crosses
	// module boundaries is the finding.
	Namespaces int `json:"namespaces"`
}

func (c Cycle) CrossModule() bool { return c.Namespaces > 1 }

// CityStats is surfaced in the UI so the analysis is auditable: what was
// skipped, what was inferred, how much the LLM was used.
type CityStats struct {
	TotalBuildings  int            `json:"total_buildings"`
	TotalTypes      int            `json:"total_types"`
	TotalModules    int            `json:"total_modules"`
	TotalMethods    int            `json:"total_methods"`
	TotalFiles      int            `json:"total_files"`
	Languages       map[string]int `json:"languages"`
	TotalLOC        int            `json:"total_loc"`
	MethodsBySyntax int            `json:"methods_by_syntax"`
	MethodsByRule   int            `json:"methods_by_rule"`
	MethodsByLLM    int            `json:"methods_by_llm"`
	Orphans         int            `json:"orphans"`
	SkippedDirs     []string       `json:"skipped_dirs"`
	LLMCalls        int            `json:"llm_calls"`
	LLMCacheHits    int            `json:"llm_cache_hits"`
	FirstCommit     string         `json:"first_commit"`
	LastCommit      string         `json:"last_commit"`
	HistoryDays     int            `json:"history_days"`
	ExtractMillis   int64          `json:"extract_millis"`
	SemanticMillis  int64          `json:"semantic_millis"`
	Refined         bool           `json:"refined"`
	Cycles          []Cycle        `json:"cycles"`
}

// CityMap is the payload consumed by the Rust/Wasm renderer.
type CityMap struct {
	Districts    []District       `json:"districts"`
	Dependencies []DependencyEdge `json:"dependencies"`
	Stats        CityStats        `json:"stats"`
}
