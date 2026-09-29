package analyzer

import "github.com/repolis/repolis/backend/internal/analyzer/lang"

// RawStruct is a composite type *definition*. Forward declarations and bare
// references are never recorded: `struct Level *p;` is also a struct_specifier,
// and recording it overwrites the real definition.
type RawStruct struct {
	Name string `json:"name"`
	// A module path, or the file path in languages without modules.
	Namespace   string   `json:"namespace"`
	SourceFile  string   `json:"source_file"`
	Fields      []string `json:"fields"`
	FieldTypes  []string `json:"field_types"`
	LinesOfCode int      `json:"lines_of_code"`
}

// RawFunction carries what the ladder and call graph need, with nothing
// language-specific left.
type RawFunction struct {
	Name      string `json:"name"`
	Namespace string `json:"namespace"`
	// The type this is a method of, where syntax says so. Empty for C, which
	// is why the association ladder exists.
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

// FileInfo is per-file metadata; git history comes from one repo-wide pass.
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

// RawExtraction is the complete output of parsing, untouched by any LLM.
type RawExtraction struct {
	Files       []FileInfo    `json:"files"`
	Structs     []RawStruct   `json:"structs"`
	Functions   []RawFunction `json:"functions"`
	SkippedDirs []string      `json:"skipped_dirs"`
	// Files per language, for the UI and diagnostics.
	Languages map[string]int `json:"languages"`
}

// DominantLanguage is what the prompts name, so a Rust project is never
// described to the model as C.
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
