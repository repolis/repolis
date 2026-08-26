package models

import "time"

// Building represents a single struct/type as a 3D building in the city.
//
// Visual mapping (CodeCity paper, Wettel & Lanza 2007):
//   - Height     → NumMethods  (NOM: tall = lots of behavior)
//   - Footprint  → NumFields   (NOA: wide = lots of state)
//   - Color/Hue  → Typology    (semantic purpose from parent district)
//   - Brightness → Age         (derived from LastModified)
//   - Texture    → CommitChurn (weathered = high churn, clean = stable)
type Building struct {
	Name         string    `json:"name"`
	SourceFile   string    `json:"source_file"`
	NumFields    int       `json:"num_fields"`
	NumMethods   int       `json:"num_methods"`
	Fields       []string  `json:"fields"`
	Methods      []string  `json:"methods"`
	LinesOfCode  int       `json:"lines_of_code"`
	CommitChurn  int       `json:"commit_churn"`
	LastModified time.Time `json:"last_modified"`
	Summary      string    `json:"summary"`
}

// District is a semantic neighborhood — NOT a filesystem directory.
// Buildings are grouped here by logical purpose (e.g., "Memory Management",
// "Networking", "Parsing & Syntax") as determined by the LLM.
//
// Visual mapping:
//   - Ground color → Typology  (core=steel, data=green, network=blue, etc.)
//   - Area         → computed from contained buildings
//   - Label        → Name
type District struct {
	Name      string     `json:"name"`
	Typology  string     `json:"typology"`
	Summary   string     `json:"summary"`
	Tags      []string   `json:"tags"`
	Buildings []Building `json:"buildings"`
}

// Road represents a filesystem directory path. Roads form the
// transportation grid around and between districts. They do NOT
// determine district membership — they show where code physically
// lives on disk.
//
// Visual mapping:
//   - Width      → FileCount (busier roads = more files)
//   - Depth      → nesting level from repo root
//   - Label      → Name (directory basename or relative path)
type Road struct {
	Name      string   `json:"name"`
	FullPath  string   `json:"full_path"`
	Depth     int      `json:"depth"`
	FileCount int      `json:"file_count"`
	Buildings []string `json:"buildings"` // building names along this road
}

// DependencyEdge represents a code-level dependency (import/include)
// between two buildings. The Rust engine renders these as glowing
// "nervous system" roads whose width scales with Weight.
type DependencyEdge struct {
	Source string `json:"source"` // building name (source of the dependency)
	Target string `json:"target"` // building name (target of the dependency)
	Weight int    `json:"weight"` // strength: number of call sites or import references
}

// CityMap is the top-level output consumed by the Rust/Wasm renderer.
type CityMap struct {
	Districts    []District       `json:"districts"`
	Roads        []Road           `json:"roads"`
	Dependencies []DependencyEdge `json:"dependencies"`
}
