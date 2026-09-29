package analyzer

import (
	"path/filepath"
	"sort"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
)

// Confidence levels produced by the deterministic ladder.
const (
	ConfHigh   = "high"
	ConfMedium = "medium"
	ConfLow    = "low"
	ConfNone   = "none"
)

// Association sources, weakest last.
const (
	SourceSyntax = "syntax" // the language stated it: `impl` block, Go receiver
	SourceRule   = "rule"   // a deterministic heuristic decided it; C only
	SourceLLM    = "llm"    // the heuristics disagreed and a model broke the tie
	SourceNone   = "none"
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

// Assoc is one function's association decision.
type Assoc struct {
	Fn         *RawFunction
	Candidates []string // TypeEntry IDs, closed set shown to the LLM
	Chosen     string   // TypeEntry ID, "" means orphan
	Confidence string
	Source     string // "rule" | "llm" | "none"
}

// Receiver is the syntactic owner the language reported, if any.
func (a *Assoc) Receiver() string { return a.Fn.Receiver }

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
	threshold := len(funcs) / 4
	if threshold < 8 {
		threshold = 8
	}

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

// prefixScoreIdx returns the length of type i's normalised name when the
// function name starts with it, with the repo prefix optionally stripped.
// Names are normalised once up front, so this allocates nothing.
func (st *SymbolTable) prefixScoreIdx(fnNorm, fnStripped string, i int) int {
	t := st.normNames[i]
	if len(t) < 3 {
		return 0
	}
	if strings.HasPrefix(fnNorm, t) {
		return len(t)
	}
	if fnStripped != "" && strings.HasPrefix(fnStripped, t) {
		return len(t)
	}
	return 0
}

func (st *SymbolTable) fnForms(fnName string) (string, string) {
	fnNorm := norm(fnName)
	fnStripped := ""
	if st.repoPrefix != "" && strings.HasPrefix(fnName, st.repoPrefix) {
		fnStripped = norm(strings.TrimPrefix(fnName, st.repoPrefix))
	}
	return fnNorm, fnStripped
}

// prefixScore is the single-pair form, used by the verifier.
func (st *SymbolTable) prefixScore(fnName, typeName string) int {
	if len(typeName) < 3 {
		return 0
	}
	t := norm(typeName)
	fnNorm, fnStripped := st.fnForms(fnName)
	if strings.HasPrefix(fnNorm, t) {
		return len(t)
	}
	if fnStripped != "" && strings.HasPrefix(fnStripped, t) {
		return len(t)
	}
	return 0
}

// bestPrefixType finds the longest type name that prefixes this function name,
// resolved to a concrete definition reachable from the function's file.
func (st *SymbolTable) bestPrefixType(fn *RawFunction) (string, int) {
	fnNorm, fnStripped := st.fnForms(fn.Name)
	bestIdx, bestPS := -1, 0
	for i := range st.Types {
		if ps := st.prefixScoreIdx(fnNorm, fnStripped, i); ps > bestPS {
			bestIdx, bestPS = i, ps
		}
	}
	if bestIdx < 0 {
		return "", 0
	}
	return st.resolve(st.Types[bestIdx].Name, fn.SourceFile), bestPS
}

// candidateSet is the closed list the LLM chooses from. Small and AST-derived
// is what stops a small model hallucinating: it cannot name a type it was not
// shown.
func (st *SymbolTable) candidateSet(fn *RawFunction) []string {
	type cand struct {
		id    string
		score int
	}
	seen := make(map[string]int)

	add := func(id string, score int) {
		if id == "" {
			return
		}
		if s, ok := seen[id]; !ok || score > s {
			seen[id] = score
		}
	}

	// Parameter types, first parameter weighted highest.
	for i, pt := range fn.ParamTypes {
		s := 40
		if i == 0 {
			s = 100
		} else if i > 2 {
			s = 20
		}
		add(st.resolve(pt, fn.SourceFile), s)
	}
	add(st.resolve(fn.ReturnType, fn.SourceFile), 60)

	// Types referenced in the body, weighted by how often.
	freq := make(map[string]int)
	for _, t := range fn.TypesUsed {
		freq[st.resolveRef(t, fn.SourceFile)]++
	}
	for id, n := range freq {
		sc := 10 + n
		if sc > 35 {
			sc = 35
		}
		add(id, sc)
	}

	// Types defined in the same file.
	for _, i := range st.byFile[fn.SourceFile] {
		add(st.Types[i].ID, 25)
	}

	// Anything the function name prefixes.
	if id, ps := st.bestPrefixType(fn); ps > 0 {
		add(id, 90+ps)
	}

	list := make([]cand, 0, len(seen))
	for id, sc := range seen {
		list = append(list, cand{id, sc})
	}
	sort.Slice(list, func(i, j int) bool {
		if list[i].score != list[j].score {
			return list[i].score > list[j].score
		}
		return list[i].id < list[j].id
	})
	if len(list) > 6 {
		list = list[:6]
	}
	out := make([]string, 0, len(list))
	for _, c := range list {
		out = append(out, c.id)
	}
	return out
}

func (st *SymbolTable) associate(fn *RawFunction) *Assoc {
	a := &Assoc{Fn: fn, Confidence: ConfNone, Source: SourceNone}

	// R0: the language already said so. Everything below exists only because
	// C has no such construct.
	if fn.Receiver != "" {
		if id := st.resolve(fn.Receiver, fn.SourceFile); id != "" {
			a.Chosen, a.Confidence, a.Source = id, ConfHigh, SourceSyntax
		}
		return a
	}

	// No receiver, in a language that has them, means genuinely free. C's
	// heuristics would attach `fn squarify(&[Item])` to Item on its first
	// parameter alone, inventing a method the language never declared.
	if st.syntactic[fn.SourceFile] {
		return a
	}

	a.Candidates = st.candidateSet(fn)
	if len(a.Candidates) == 0 {
		return a
	}

	// R1: longest type-name prefix of the function name.
	byPrefix, _ := st.bestPrefixType(fn)

	// R2: first parameter type — the dominant "method receiver" convention in C.
	byParam := ""
	if len(fn.ParamTypes) > 0 {
		byParam = st.resolve(fn.ParamTypes[0], fn.SourceFile)
	}

	// R3: return type (constructor idiom).
	byReturn := st.resolve(fn.ReturnType, fn.SourceFile)

	// R4: most frequently referenced type in the body.
	byBody := ""
	{
		freq := make(map[string]int)
		for _, t := range fn.TypesUsed {
			if id := st.resolveRef(t, fn.SourceFile); id != "" {
				freq[id]++
			}
		}
		bestN := 0
		for id, n := range freq {
			if n > bestN || (n == bestN && id < byBody) {
				byBody, bestN = id, n
			}
		}
	}

	switch {
	case byPrefix != "" && byParam != "" && byPrefix == byParam:
		a.Chosen, a.Confidence, a.Source = byPrefix, ConfHigh, SourceRule
	case byPrefix != "" && byParam != "" && byPrefix != byParam:
		// Genuine ambiguity, but a tentative answer is still recorded so an
		// unavailable or budgeted-out LLM does not orphan the function. The
		// name prefix is the better prior in C; the LLM may overturn it.
		a.Chosen, a.Confidence, a.Source = byPrefix, ConfLow, SourceRule
	case byParam != "":
		a.Chosen, a.Confidence, a.Source = byParam, ConfHigh, SourceRule
	case byPrefix != "":
		a.Chosen, a.Confidence, a.Source = byPrefix, ConfMedium, SourceRule
	case byReturn != "":
		a.Chosen, a.Confidence, a.Source = byReturn, ConfMedium, SourceRule
	case byBody != "":
		a.Chosen, a.Confidence, a.Source = byBody, ConfLow, SourceRule
	default:
		a.Confidence = ConfNone
	}

	if a.Chosen != "" && !st.verify(fn, a.Chosen) {
		a.Chosen, a.Confidence, a.Source = "", ConfLow, SourceNone
	}
	return a
}

// verify is the check every association must pass, the LLM's included: an
// attribution no AST evidence supports would silently inflate a building.
func (st *SymbolTable) verify(fn *RawFunction, typeID string) bool {
	idx, ok := st.byID[typeID]
	if !ok {
		return false
	}
	name := st.Types[idx].Name

	if st.prefixScore(fn.Name, name) > 0 {
		return true
	}
	for _, pt := range fn.ParamTypes {
		if pt == name {
			return true
		}
	}
	if fn.ReturnType == name {
		return true
	}
	for _, t := range fn.TypesUsed {
		if t.Name == name {
			return true
		}
	}
	return false
}

// Residual returns what the rules could not settle, most consequential first.
// Every entry already carries a tentative answer, so a caller may stop early
// and trade accuracy for latency without losing a function. Impact is the
// state the competing types hold, since that is what a wrong answer distorts.
func (st *SymbolTable) Residual() []*Assoc {
	var out []*Assoc
	for _, a := range st.Assocs {
		if a.Confidence == ConfLow && len(a.Candidates) >= 2 {
			out = append(out, a)
		}
	}
	impact := func(a *Assoc) int {
		n := 0
		for _, cid := range a.Candidates {
			if idx, ok := st.byID[cid]; ok {
				n += len(st.Types[idx].Fields)
			}
		}
		return n
	}
	sort.Slice(out, func(i, j int) bool {
		ii, ij := impact(out[i]), impact(out[j])
		if ii != ij {
			return ii > ij
		}
		if out[i].Fn.SourceFile != out[j].Fn.SourceFile {
			return out[i].Fn.SourceFile < out[j].Fn.SourceFile
		}
		return out[i].Fn.Name < out[j].Fn.Name
	})
	return out
}

// SymbolPrefix is the dominant function-name prefix of the codebase.
func (st *SymbolTable) SymbolPrefix() string { return st.repoPrefix }

func (st *SymbolTable) TypeName(id string) string {
	if idx, ok := st.byID[id]; ok {
		return st.Types[idx].Name
	}
	return ""
}

// ApplyLLM records an adjudicated choice, after the same verification.
func (st *SymbolTable) ApplyLLM(a *Assoc, chosenID string) bool {
	if chosenID == "" {
		return false
	}
	valid := false
	for _, c := range a.Candidates {
		if c == chosenID {
			valid = true
			break
		}
	}
	if !valid || !st.verify(a.Fn, chosenID) {
		return false
	}
	changed := a.Chosen != chosenID
	a.Chosen, a.Confidence, a.Source = chosenID, ConfMedium, SourceLLM
	return changed
}

// ClearTentative unattaches a function: the model answered NONE, which is an
// answer rather than a failure.
func (st *SymbolTable) ClearTentative(a *Assoc) {
	a.Chosen, a.Confidence, a.Source = "", ConfNone, SourceNone
}
