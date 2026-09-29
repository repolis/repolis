package analyzer

import "github.com/repolis/repolis/backend/internal/analyzer/lang"

// RawStruct is a *definition* of a composite type. Forward declarations and
// bare type references are deliberately never recorded: in C, `struct Level *p;`
// also contains a struct_specifier, and treating those as definitions used to
// overwrite the real ones.
type RawStruct struct {
	Name string `json:"name"`
	// Namespace is the scope the type is defined in: a module path in
	// languages that have one, the file path in languages that do not.
	Namespace   string   `json:"namespace"`
	SourceFile  string   `json:"source_file"`
	Fields      []string `json:"fields"`
	FieldTypes  []string `json:"field_types"`
	LinesOfCode int      `json:"lines_of_code"`
}

// RawFunction carries everything the association ladder and the call graph
// need. None of it is language specific by the time it gets here.
type RawFunction struct {
	Name      string `json:"name"`
	Namespace string `json:"namespace"`
	// Receiver is the type this function is a method of, when the language
	// says so syntactically (Rust `impl`, Go receivers, class bodies). C
	// leaves it empty, which is why C needs the association ladder at all.
	Receiver    string     `json:"receiver"`
	SourceFile  string     `json:"source_file"`
	Signature   string     `json:"signature"`
	ReturnType  string     `json:"return_type"`
	ParamTypes  []string   `json:"param_types"`
	TypesUsed   []lang.Ref `json:"types_used"`
	Calls       []lang.Ref `json:"calls"`
	Complexity  int        `json:"complexity"`
	LinesOfCode int        `json:"lines_of_code"`
}

// FileInfo is per-file metadata. Git history is filled in by a single
// repo-wide `git log` pass (internal/git/history.go).
type FileInfo struct {
	Path          string        `json:"path"`
	Dir           string        `json:"dir"`
	Extension     string        `json:"extension"`
	Language      string        `json:"language"`
	Namespace     string        `json:"namespace"`
	Depth         int           `json:"depth"`
	LinesOfCode   int           `json:"lines_of_code"`
	FileScopeVars int           `json:"file_scope_vars"`
	Imports       []lang.Import `json:"imports"`
	CommitChurn   int           `json:"commit_churn"`
	LastModified  string        `json:"last_modified"`
	PrimaryAuthor string        `json:"primary_author"`
}

// RawExtraction is the complete output of parsing. Nothing here has touched
// an LLM.
type RawExtraction struct {
	Files       []FileInfo    `json:"files"`
	Structs     []RawStruct   `json:"structs"`
	Functions   []RawFunction `json:"functions"`
	SkippedDirs []string      `json:"skipped_dirs"`
	// Languages counts files per language, for the UI and for diagnostics.
	Languages map[string]int `json:"languages"`
}

// DominantLanguage is the language most of the repository is written in. It
// is what the LLM prompts name, so a Rust project is never described to the
// model as C.
func (r *RawExtraction) DominantLanguage() string {
	best, bestN := "", 0
	for name, n := range r.Languages {
		if n > bestN || (n == bestN && name < best) {
			best, bestN = name, n
		}
	}
	if best == "" {
		return "source"
	}
	return best
}
