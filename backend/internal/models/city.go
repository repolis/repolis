package models

// Building is one node of the city: either a concrete type (struct/union/enum
// definition) or a "module" standing in for the free functions of a file.
//
// Visual mapping consumed by the Rust engine:
//   - Height      -> NumMethods   (behaviour)
//   - Footprint   -> NumFields    (state)
//   - Hue         -> parent district Typology
//   - Saturation  -> AgeDays      (old code desaturates)
//   - Roof glow   -> ChurnRank    (actively edited code glows)
//   - Roof slab   -> Kind         ("module" gets a contrasting cap)
type Building struct {
	// ID is globally unique and stable: "<source_file>::<name>".
	// Dependency edges reference buildings by ID, never by Name, because C
	// happily declares two different file-scoped structs with the same name.
	ID         string `json:"id"`
	Name       string `json:"name"`
	Kind       string `json:"kind"` // "type" | "module"
	SourceFile string `json:"source_file"`
	Dir        string `json:"dir"`
	// Namespace is the module or package the symbol belongs to.
	Namespace string `json:"namespace"`
	Language  string `json:"language"`

	NumFields   int      `json:"num_fields"`
	NumMethods  int      `json:"num_methods"`
	Fields      []string `json:"fields"`
	Methods     []string `json:"methods"`
	LinesOfCode int      `json:"lines_of_code"`

	// MaxComplexity is the worst cyclomatic complexity among this building's
	// functions, SumComplexity their total. Max answers "is there a monster
	// in here", which is the question that usually matters; sum answers "how
	// much branching is there overall".
	MaxComplexity int `json:"max_complexity"`
	SumComplexity int `json:"sum_complexity"`

	// FanIn is how many other buildings depend on this one, FanOut how many
	// it depends on.
	FanIn  int `json:"fan_in"`
	FanOut int `json:"fan_out"`
	// Instability is Martin's I = out / (in + out): 0 means nothing depends
	// on anything here except this, 1 means this depends on everything and
	// nothing depends on it. Zero when the building has no edges at all.
	Instability float64 `json:"instability"`
	// Hub marks a building in the top few percent by fan-in.
	Hub bool `json:"hub"`

	CommitChurn  int     `json:"commit_churn"`
	ChurnRank    float64 `json:"churn_rank"` // 0..1 percentile within the repo
	LastModified string  `json:"last_modified"`
	// FirstSeen is when this file first appeared, which drives the timeline.
	FirstSeen string `json:"first_seen"`
	// AgeAtBirthDays is days between the repository's first commit and this
	// building's, so the timeline needs no absolute dates at render time.
	BornDay       int    `json:"born_day"`
	AgeDays       int    `json:"age_days"`
	PrimaryAuthor string `json:"primary_author"`

	Summary string `json:"summary"`

	// CycleID is the dependency cycle this building belongs to, or 0. Cycles
	// are numbered from 1, largest first.
	CycleID int `json:"cycle_id"`

	// AssocSource records how this building's methods were attributed:
	// "rule" (deterministic AST rules), "llm" (adjudicated), "none".
	// Surfaced in the inspector so inferred data is never mistaken for fact.
	AssocSource string `json:"assoc_source"`
}

// District is a semantic neighbourhood produced by graph clustering over the
// call graph plus directory affinity, then named by the LLM. It is NOT a
// filesystem directory, though directories dominate the initial labelling.
type District struct {
	ID        string     `json:"id"`
	Name      string     `json:"name"`
	Typology  string     `json:"typology"`
	Summary   string     `json:"summary"`
	Tags      []string   `json:"tags"`
	Buildings []Building `json:"buildings"`
}

// DependencyEdge is a call-site-accurate edge between two buildings.
// Kind is "call" (function call) or "type" (struct embeds/references struct).
type DependencyEdge struct {
	Source string `json:"source"` // building ID
	Target string `json:"target"` // building ID
	Weight int    `json:"weight"` // number of call sites / references
	Kind   string `json:"kind"`
	// InCycle is true when both ends sit in the same dependency cycle.
	InCycle bool `json:"in_cycle"`
}

// Cycle is one strongly connected component of the call graph with more than
// one member: a group of buildings that all, directly or indirectly, depend on
// each other.
type Cycle struct {
	ID      int      `json:"id"`
	Size    int      `json:"size"`
	Members []string `json:"members"`
	// Reliable marks a cycle small enough to be trusted as a finding.
	//
	// The call graph is heuristic: a method call gives a name but not a
	// receiver type, so a single wrong edge merges two components. Small
	// components survive that - every 2-to-5 building cycle spot-checked
	// against the source was real - while a component of dozens is as likely
	// to be an artefact of one bad edge as a genuine tangle. Reporting both
	// with the same confidence would be dishonest.
	Reliable bool `json:"reliable"`

	// Namespaces is how many distinct modules the cycle spans.
	//
	// A cycle inside one module is structurally normal - every file of a Go
	// package can see every other, and cobra's eight mutually referencing
	// files are all package `cobra`. A cycle that crosses module boundaries
	// is the one worth reporting, because it means two units that were meant
	// to be separable are not.
	Namespaces int `json:"namespaces"`
}

// CrossModule reports whether this cycle spans more than one module.
func (c Cycle) CrossModule() bool { return c.Namespaces > 1 }

// CityStats is diagnostic data surfaced in the UI so the analysis is auditable:
// how much was skipped, how much was inferred, how much the LLM was used.
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
