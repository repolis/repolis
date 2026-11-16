// Symbol table: every type, where it is visible from, and name resolution.

package analyzer

import (
	"path/filepath"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
)

// TypeEntry is one composite-type definition, keyed by a globally unique ID.
type TypeEntry struct {
	ID         string
	Name       string
	Namespace  string
	SourceFile string
	Dir        string
	Fields     []string
	FieldTypes []string
	LOC        int
}

// SymbolTable is the resolved view of a repository's types and functions.
type SymbolTable struct {
	Types      []TypeEntry
	byID       map[string]int
	byName     map[string][]int
	byFile     map[string][]int
	syntactic  map[string]bool // file -> language states methods in syntax
	normNames  []string
	Assocs     []*Assoc
	repoPrefix string
	scopes     *scopes
	language   string
}

// Language is the repository's dominant language, used to word prompts.
func (st *SymbolTable) Language() string { return st.language }

func typeID(file, name string) string { return file + "::" + name }
func moduleID(file string) string     { return file + "::<module>" }

func dirOf(path string) string {
	d := filepath.ToSlash(filepath.Dir(path))
	if d == "." || d == "/" || d == "" {
		return "root"
	}
	return d
}

// norm makes snake_case and camelCase spellings compare equal.
func norm(s string) string {
	return strings.ToLower(strings.ReplaceAll(s, "_", ""))
}

// detectRepoPrefix finds the dominant function-name prefix ("git_", "sqlite3").
// Stripping it is what connects `sqlite3VdbeAddOp3` to the type `Vdbe`.
func detectRepoPrefix(funcs []RawFunction) string {
	if len(funcs) < 12 {
		return ""
	}
	threshold := max(len(funcs)/4, 8)

	best := ""
	for l := 3; l <= 12; l++ {
		counts := make(map[string]int)
		for i := range funcs {
			if len(funcs[i].Name) > l {
				counts[funcs[i].Name[:l]]++
			}
		}
		top, topN := "", 0
		for p, n := range counts {
			if n > topN || (n == topN && p < top) {
				top, topN = p, n
			}
		}
		if topN >= threshold {
			best = top // keep extending while the threshold still holds
		}
	}
	return best
}

// BuildSymbolTable resolves every type definition and runs the deterministic
// association ladder over every function.
func BuildSymbolTable(raw *RawExtraction) *SymbolTable {
	st := &SymbolTable{
		byID:      make(map[string]int),
		byName:    make(map[string][]int),
		byFile:    make(map[string][]int),
		syntactic: make(map[string]bool),
	}
	for _, f := range raw.Files {
		if l := lang.ForFile(f.Path); l != nil {
			st.syntactic[f.Path] = l.SyntacticMethods()
		}
	}

	// Types, de-duplicated by ID (an #ifdef can define one name twice in a
	// file); the richest definition wins.
	for _, s := range raw.Structs {
		id := typeID(s.SourceFile, s.Name)
		if idx, ok := st.byID[id]; ok {
			if len(s.Fields) > len(st.Types[idx].Fields) {
				st.Types[idx].Fields = s.Fields
				st.Types[idx].FieldTypes = s.FieldTypes
				st.Types[idx].LOC = s.LinesOfCode
			}
			continue
		}
		st.byID[id] = len(st.Types)
		st.Types = append(st.Types, TypeEntry{
			ID: id, Name: s.Name, Namespace: s.Namespace,
			SourceFile: s.SourceFile, Dir: dirOf(s.SourceFile),
			Fields: s.Fields, FieldTypes: s.FieldTypes, LOC: s.LinesOfCode,
		})
	}
	st.normNames = make([]string, len(st.Types))
	for i := range st.Types {
		st.byName[st.Types[i].Name] = append(st.byName[st.Types[i].Name], i)
		st.byFile[st.Types[i].SourceFile] = append(st.byFile[st.Types[i].SourceFile], i)
		st.normNames[i] = norm(st.Types[i].Name)
	}

	st.scopes = buildScopes(raw)
	st.language = raw.DominantLanguage()
	st.repoPrefix = detectRepoPrefix(raw.Functions)

	// 2. Association ladder.
	for i := range raw.Functions {
		st.Assocs = append(st.Assocs, st.associate(&raw.Functions[i]))
	}

	return st
}

// resolve maps a bare type name to a definition, preferring one the caller can
// actually see according to the import graph.
func (st *SymbolTable) resolve(name, fromFile string) string {
	return st.resolveIn(name, fromFile, nil)
}

// resolveIn is resolve, restricted to the namespaces a qualifier named.
func (st *SymbolTable) resolveIn(name, fromFile string, namespaces []string) string {
	if name == "" {
		return ""
	}
	idxs := st.byName[name]
	if len(idxs) == 0 {
		return ""
	}

	if len(namespaces) > 0 {
		allowed := make(map[string]bool, len(namespaces))
		for _, ns := range namespaces {
			allowed[ns] = true
		}
		best, bestRank := "", -1
		for _, idx := range idxs {
			t := st.Types[idx]
			if !allowed[t.Namespace] {
				continue
			}
			r := st.scopes.rank(fromFile, t.Namespace)
			if r > bestRank || (r == bestRank && t.ID < best) {
				best, bestRank = t.ID, r
			}
		}
		if best != "" {
			return best
		}
		// A qualifier matching no known namespace is external; fall through
		// rather than inventing a local match.
	}

	if len(idxs) == 1 {
		return st.Types[idxs[0]].ID
	}
	best, bestRank := "", -1
	for _, idx := range idxs {
		t := st.Types[idx]
		r := st.scopes.rank(fromFile, t.Namespace)
		if t.SourceFile == fromFile {
			r++ // same file beats same namespace
		}
		if r > bestRank || (r == bestRank && t.ID < best) {
			best, bestRank = t.ID, r
		}
	}
	return best
}

// resolveRef resolves a reference that may carry a written qualifier.
func (st *SymbolTable) resolveRef(ref lang.Ref, fromFile string) string {
	if ref.Qualifier == "" {
		// The name may itself be a local alias for an imported symbol.
		if real, namespaces := st.scopes.aliasTarget(ref.Name, fromFile); real != "" {
			if id := st.resolveIn(real, fromFile, namespaces); id != "" {
				return id
			}
		}
		return st.resolve(ref.Name, fromFile)
	}
	return st.resolveIn(ref.Name, fromFile, st.scopes.expand(ref.Qualifier, fromFile))
}

// SymbolPrefix is the dominant function-name prefix of the codebase.
func (st *SymbolTable) SymbolPrefix() string { return st.repoPrefix }

func (st *SymbolTable) TypeName(id string) string {
	if idx, ok := st.byID[id]; ok {
		return st.Types[idx].Name
	}
	return ""
}
