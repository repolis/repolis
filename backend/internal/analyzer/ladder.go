package analyzer

import (
	"cmp"
	"slices"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
)

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

type Assoc struct {
	Fn         *RawFunction
	Candidates []string // TypeEntry IDs, closed set shown to the LLM
	Chosen     string   // TypeEntry ID, "" means orphan
	Confidence string
	Source     string // "rule" | "llm" | "none"
}

func (a *Assoc) Receiver() string { return a.Fn.Receiver }

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

// The closed, AST-derived set stops a small model hallucinating: it cannot name
// a type it was not shown.
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

	freq := make(map[string]int)
	for _, t := range fn.TypesUsed {
		freq[st.resolveRef(t, fn.SourceFile)]++
	}
	for id, n := range freq {
		add(id, min(10+n, 35))
	}

	for _, i := range st.byFile[fn.SourceFile] {
		add(st.Types[i].ID, 25)
	}

	if id, ps := st.bestPrefixType(fn); ps > 0 {
		add(id, 90+ps)
	}

	list := make([]cand, 0, len(seen))
	for id, sc := range seen {
		list = append(list, cand{id, sc})
	}
	slices.SortFunc(list, func(a, b cand) int {
		return cmp.Or(cmp.Compare(b.score, a.score), strings.Compare(a.id, b.id))
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

	// The language said so. Everything below exists only because C has no such
	// construct.
	if fn.Receiver != "" {
		if id := st.resolve(fn.Receiver, fn.SourceFile); id != "" {
			a.Chosen, a.Confidence, a.Source = id, ConfHigh, SourceSyntax
		}
		return a
	}

	// No receiver in a language that has them means genuinely free; C's rules
	// would attach `fn squarify(&[Item])` to Item and invent a method.
	if st.syntactic[fn.SourceFile] {
		return a
	}

	a.Candidates = st.candidateSet(fn)
	if len(a.Candidates) == 0 {
		return a
	}

	byPrefix, _ := st.bestPrefixType(fn)

	byParam := ""
	if len(fn.ParamTypes) > 0 {
		byParam = st.resolve(fn.ParamTypes[0], fn.SourceFile)
	}

	byReturn := st.resolve(fn.ReturnType, fn.SourceFile)

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
		// Genuine ambiguity, but record a tentative answer so a missing or
		// budgeted-out LLM does not orphan the function. Name prefix is the
		// better prior in C.
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

// Every association must pass this, the LLM's included: an attribution without
// AST evidence would silently inflate a building.
func (st *SymbolTable) verify(fn *RawFunction, typeID string) bool {
	idx, ok := st.byID[typeID]
	if !ok {
		return false
	}
	name := st.Types[idx].Name

	return st.prefixScore(fn.Name, name) > 0 ||
		slices.Contains(fn.ParamTypes, name) ||
		fn.ReturnType == name ||
		slices.ContainsFunc(fn.TypesUsed, func(t lang.Ref) bool { return t.Name == name })
}

// Most consequential first; every entry already carries a tentative answer, so
// callers may stop early. Impact is the state the competing types hold.
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
	slices.SortFunc(out, func(a, b *Assoc) int {
		return cmp.Or(cmp.Compare(impact(b), impact(a)),
			strings.Compare(a.Fn.SourceFile, b.Fn.SourceFile),
			strings.Compare(a.Fn.Name, b.Fn.Name))
	})
	return out
}

type LLMResult int

const (
	LLMRejected  LLMResult = iota // not a candidate, or no AST evidence for it
	LLMUnchanged                  // confirmed the rule's choice
	LLMApplied                    // replaced the rule's choice
)

func (st *SymbolTable) ApplyLLM(a *Assoc, chosenID string) LLMResult {
	if !slices.Contains(a.Candidates, chosenID) || !st.verify(a.Fn, chosenID) {
		return LLMRejected
	}
	changed := a.Chosen != chosenID
	a.Chosen, a.Confidence, a.Source = chosenID, ConfMedium, SourceLLM
	if changed {
		return LLMApplied
	}
	return LLMUnchanged
}

// The model answered NONE, which is an answer rather than a failure.
func (a *Assoc) Detach() {
	a.Chosen, a.Confidence, a.Source = "", ConfNone, SourceNone
}
