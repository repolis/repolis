package analyzer

import (
	"cmp"
	"slices"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
	"github.com/repolis/repolis/backend/internal/models"
)

// Per building, not globally: a global top-K keeps only the densest hubs and
// drops local structure everywhere else.
const maxOutPerBuilding = 8

type edgeKey struct {
	src, tgt string
	kind     string
}

type calleeResolver struct {
	st      *SymbolTable
	ownerOf map[string]string
	byName  map[string][]*RawFunction
	// By (receiver, name) too, so `Camera::new` picks the right one of several
	// same-named associated functions.
	byRecv map[string][]*RawFunction
}

func newCalleeResolver(raw *RawExtraction, st *SymbolTable, ownerOf map[string]string) *calleeResolver {
	r := &calleeResolver{
		st:      st,
		ownerOf: ownerOf,
		byName:  make(map[string][]*RawFunction),
		byRecv:  make(map[string][]*RawFunction),
	}
	for i := range raw.Functions {
		f := &raw.Functions[i]
		r.byName[f.Name] = append(r.byName[f.Name], f)
		if f.Receiver != "" {
			r.byRecv[f.Receiver+"::"+f.Name] = append(r.byRecv[f.Receiver+"::"+f.Name], f)
		}
	}
	return r
}

// In a module-scoped language an unimported symbol cannot be called, so
// zero visibility is no match at all. Accepting those invented edges that
// ran backwards through ripgrep's crate graph and fused a third of the
// city into one "cycle".
func (r *calleeResolver) pick(defs []*RawFunction, fromFile string) string {
	if len(defs) == 0 {
		return ""
	}
	strict := r.st.scopes.IsModuleScoped(fromFile)
	if len(defs) == 1 {
		if strict && r.st.scopes.rank(fromFile, defs[0].Namespace) < visImported {
			return ""
		}
		return r.ownerOf[funcKey(defs[0])]
	}
	best, bestRank := "", -1
	tiedAtBest := 0
	for _, d := range defs {
		rank := r.st.scopes.rank(fromFile, d.Namespace)
		if d.SourceFile == fromFile {
			rank++
		}
		owner := r.ownerOf[funcKey(d)]
		if rank > bestRank {
			best, bestRank, tiedAtBest = owner, rank, 1
		} else if rank == bestRank {
			tiedAtBest++
			if owner < best {
				best = owner
			}
		}
	}
	// Several equally invisible definitions carry no information, and a
	// wrong edge is worse than a missing one.
	if strict && bestRank < visImported {
		return ""
	}
	if bestRank <= visAnywhere && tiedAtBest > 3 {
		return ""
	}
	return best
}

func (r *calleeResolver) scoped(name string, namespaces []string, fromFile string) string {
	if len(namespaces) == 0 {
		return ""
	}
	allowed := make(map[string]bool, len(namespaces))
	for _, ns := range namespaces {
		allowed[ns] = true
	}
	var defs []*RawFunction
	for _, d := range r.byName[name] {
		if allowed[d.Namespace] {
			defs = append(defs, d)
		}
	}
	return r.pick(defs, fromFile)
}

// A method call gives the name but never the receiver's type, so only an
// unambiguous resolution is honest. Linking `x.len()` to whichever
// definition ranked highest fused 126 of ripgrep's 355 buildings into one
// bogus component.
func (r *calleeResolver) pickUnambiguous(name, fromFile string) string {
	defs := r.byName[name]
	if len(defs) == 0 {
		return ""
	}
	if len(defs) == 1 {
		return r.ownerOf[funcKey(defs[0])]
	}
	var local []*RawFunction
	for _, d := range defs {
		if d.SourceFile == fromFile || r.st.scopes.rank(fromFile, d.Namespace) >= visOwn {
			local = append(local, d)
		}
	}
	if len(local) == 1 {
		return r.ownerOf[funcKey(local[0])]
	}
	return ""
}

func (r *calleeResolver) resolve(ref lang.Ref, fromFile string) string {
	if ref.Method {
		return r.pickUnambiguous(ref.Name, fromFile)
	}

	// An unqualified call may still be a renamed import.
	if ref.Qualifier == "" {
		if real, namespaces := r.st.scopes.aliasTarget(ref.Name, fromFile); real != "" {
			if owner := r.scoped(real, namespaces, fromFile); owner != "" {
				return owner
			}
		}
	}

	if ref.Qualifier != "" {
		// `Type::method` - the qualifier is the receiver.
		if recvType := r.st.scopes.qualifierType(ref.Qualifier, fromFile); recvType != "" {
			if defs := r.byRecv[recvType+"::"+ref.Name]; len(defs) > 0 {
				if owner := r.pick(defs, fromFile); owner != "" {
					return owner
				}
			}
		}
		// `module::func` - restrict candidates to that module.
		if owner := r.scoped(ref.Name, r.st.scopes.expand(ref.Qualifier, fromFile), fromFile); owner != "" {
			return owner
		}
	}
	// A bare name in a module-scoped language can only mean something
	// declared here or explicitly imported.
	if r.st.scopes.IsModuleScoped(fromFile) {
		own := r.st.scopes.OwnNamespace(fromFile)
		var local []*RawFunction
		for _, d := range r.byName[ref.Name] {
			if d.Namespace == own {
				local = append(local, d)
			}
		}
		if owner := r.pick(local, fromFile); owner != "" {
			return owner
		}
		return ""
	}
	return r.pick(r.byName[ref.Name], fromFile)
}

// Three calls to `foo()` from one method of Vdbe give one edge Vdbe -> owner of
// foo, weight 3.
func BuildDependencies(raw *RawExtraction, st *SymbolTable, ownerOf map[string]string) []models.DependencyEdge {
	callees := newCalleeResolver(raw, st, ownerOf)
	counts := make(map[edgeKey]int)

	for i := range raw.Functions {
		fn := &raw.Functions[i]
		src := ownerOf[funcKey(fn)]
		if src == "" {
			continue
		}
		for _, callee := range fn.Calls {
			tgt := callees.resolve(callee, fn.SourceFile)
			if tgt == "" || tgt == src {
				continue
			}
			counts[edgeKey{src, tgt, "call"}]++
		}
	}

	for _, t := range st.Types {
		for _, ft := range t.FieldTypes {
			tgt := st.resolve(ft, t.SourceFile)
			if tgt == "" || tgt == t.ID {
				continue
			}
			counts[edgeKey{t.ID, tgt, "type"}]++
		}
	}

	all := make([]models.DependencyEdge, 0, len(counts))
	for k, w := range counts {
		all = append(all, models.DependencyEdge{
			Source: k.src, Target: k.tgt, Weight: w, Kind: k.kind,
		})
	}
	return keepStrongest(all, maxOutPerBuilding)
}

func keepStrongest(edges []models.DependencyEdge, perSource int) []models.DependencyEdge {
	slices.SortFunc(edges, func(a, b models.DependencyEdge) int {
		return cmp.Or(
			strings.Compare(a.Source, b.Source),
			cmp.Compare(b.Weight, a.Weight),
			strings.Compare(a.Target, b.Target),
			strings.Compare(a.Kind, b.Kind),
		)
	})

	out := make([]models.DependencyEdge, 0, len(edges))
	kept := 0
	var lastSrc string
	for _, e := range edges {
		if e.Source != lastSrc {
			lastSrc, kept = e.Source, 0
		}
		if kept >= perSource {
			continue
		}
		kept++
		out = append(out, e)
	}
	return out
}

// The receiver is part of the key because a method name is unique per type,
// not per file: without it the two `Name()` methods in one gin file collide.
func funcKey(f *RawFunction) string {
	return f.SourceFile + "::" + f.Receiver + "#" + f.Name
}
