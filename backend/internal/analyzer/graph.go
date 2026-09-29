package analyzer

import (
	"sort"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
	"github.com/repolis/repolis/backend/internal/models"
)

// maxOutPerBuilding caps each building's outgoing edges.
//
// The original implementation expanded every #include into the full cartesian
// product of (buildings in the including file) x (buildings in the included
// file), then kept a global top-500 by weight. One header holding 104 types
// produced 520 edges from a single directive, so the surviving edges were the
// densest cartesian products rather than the most meaningful relationships.
// Capping per building keeps local structure everywhere instead of
// concentrating on hub headers.
const maxOutPerBuilding = 8

type edgeKey struct {
	src, tgt string
	kind     string
}

// BuildDependencies derives call-site-accurate edges.
//
// A call is attributed to the specific function containing it, and that
// function to its owning building, so `foo()` called three times from inside
// a method of Vdbe produces exactly one edge Vdbe -> <owner of foo> weight 3.
func BuildDependencies(raw *RawExtraction, st *SymbolTable, ownerOf map[string]string) []models.DependencyEdge {
	// Index every defined function by name, and by (receiver, name) so that a
	// qualified call such as Rust's `Camera::new` resolves to the right one of
	// several same-named associated functions.
	byName := make(map[string][]*RawFunction)
	byRecv := make(map[string][]*RawFunction)
	for i := range raw.Functions {
		f := &raw.Functions[i]
		byName[f.Name] = append(byName[f.Name], f)
		if f.Receiver != "" {
			byRecv[f.Receiver+"::"+f.Name] = append(byRecv[f.Receiver+"::"+f.Name], f)
		}
	}

	counts := make(map[edgeKey]int)

	// In a module-scoped language a symbol that was never imported cannot be
	// called, so a candidate at zero visibility is not a weak match - it is
	// not a match at all. Accepting those produced edges that ran backwards
	// through ripgrep's crate graph (searcher -> core, regex -> globset),
	// which Cargo could never have compiled, and those false edges were what
	// fused a third of the city into one "cycle".
	pick := func(defs []*RawFunction, fromFile string) string {
		if len(defs) == 0 {
			return ""
		}
		strict := st.scopes.IsModuleScoped(fromFile)
		if len(defs) == 1 {
			if strict && st.scopes.rank(fromFile, defs[0].Namespace) < visImported {
				return ""
			}
			return ownerOf[funcKey(defs[0])]
		}
		best, bestRank := "", -1
		ambiguous := 0
		for _, d := range defs {
			r := st.scopes.rank(fromFile, d.Namespace)
			if d.SourceFile == fromFile {
				r++
			}
			owner := ownerOf[funcKey(d)]
			if r > bestRank {
				best, bestRank, ambiguous = owner, r, 1
			} else if r == bestRank {
				ambiguous++
				if owner < best {
					best = owner
				}
			}
		}
		// A name defined in several equally invisible places carries no
		// information; a wrong edge is worse than a missing one.
		if strict && bestRank < visImported {
			return ""
		}
		if bestRank <= visAnywhere && ambiguous > 3 {
			return ""
		}
		return best
	}

	scoped := func(name string, namespaces []string, fromFile string) string {
		if len(namespaces) == 0 {
			return ""
		}
		allowed := make(map[string]bool, len(namespaces))
		for _, ns := range namespaces {
			allowed[ns] = true
		}
		var defs []*RawFunction
		for _, d := range byName[name] {
			if allowed[d.Namespace] {
				defs = append(defs, d)
			}
		}
		return pick(defs, fromFile)
	}

	// A method call gives us the name but never the receiver's type, so the
	// only honest resolution is an unambiguous one.
	//
	// Without this, `x.len()` or `x.new()` linked whatever definition happened
	// to rank highest, and those false edges fused 126 of ripgrep's 355
	// buildings into a single strongly connected component - a "cycle" that
	// was entirely an artefact. A missing edge is a smaller lie than an
	// invented one, especially once cycles are reported as a finding.
	pickUnambiguous := func(name, fromFile string) string {
		defs := byName[name]
		if len(defs) == 0 {
			return ""
		}
		if len(defs) == 1 {
			return ownerOf[funcKey(defs[0])]
		}
		var local []*RawFunction
		for _, d := range defs {
			if d.SourceFile == fromFile || st.scopes.rank(fromFile, d.Namespace) >= visOwn {
				local = append(local, d)
			}
		}
		if len(local) == 1 {
			return ownerOf[funcKey(local[0])]
		}
		return ""
	}

	resolveCallee := func(ref lang.Ref, fromFile string) string {
		if ref.Method {
			return pickUnambiguous(ref.Name, fromFile)
		}

		// An unqualified call may still be a renamed import.
		if ref.Qualifier == "" {
			if real, namespaces := st.scopes.aliasTarget(ref.Name, fromFile); real != "" {
				if owner := scoped(real, namespaces, fromFile); owner != "" {
					return owner
				}
			}
		}

		// A qualified call names its scope; honour it before anything else.
		if ref.Qualifier != "" {
			// `Type::method` - the qualifier is the receiver.
			if recvType := st.scopes.qualifierType(ref.Qualifier, fromFile); recvType != "" {
				if defs := byRecv[recvType+"::"+ref.Name]; len(defs) > 0 {
					if owner := pick(defs, fromFile); owner != "" {
						return owner
					}
				}
			}
			// `module::func` - restrict candidates to that module.
			if owner := scoped(ref.Name, st.scopes.expand(ref.Qualifier, fromFile), fromFile); owner != "" {
				return owner
			}
		}
		// A bare name. In a module-scoped language it can only mean something
		// declared here or explicitly imported; searching every visible
		// namespace would resolve a common helper into whichever dependency
		// happened to rank highest.
		if st.scopes.IsModuleScoped(fromFile) {
			own := st.scopes.OwnNamespace(fromFile)
			var local []*RawFunction
			for _, d := range byName[ref.Name] {
				if d.Namespace == own {
					local = append(local, d)
				}
			}
			if owner := pick(local, fromFile); owner != "" {
				return owner
			}
			return ""
		}
		return pick(byName[ref.Name], fromFile)
	}

	for i := range raw.Functions {
		fn := &raw.Functions[i]
		src := ownerOf[funcKey(fn)]
		if src == "" {
			continue
		}
		for _, callee := range fn.Calls {
			tgt := resolveCallee(callee, fn.SourceFile)
			if tgt == "" || tgt == src {
				continue
			}
			counts[edgeKey{src, tgt, "call"}]++
		}
	}

	// Type composition edges: struct A holds a struct B.
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

	// Deterministic order, then per-source top-K.
	sort.Slice(all, func(i, j int) bool {
		if all[i].Source != all[j].Source {
			return all[i].Source < all[j].Source
		}
		if all[i].Weight != all[j].Weight {
			return all[i].Weight > all[j].Weight
		}
		return all[i].Target < all[j].Target
	})

	out := make([]models.DependencyEdge, 0, len(all))
	perSrc := 0
	var lastSrc string
	for _, e := range all {
		if e.Source != lastSrc {
			lastSrc, perSrc = e.Source, 0
		}
		if perSrc >= maxOutPerBuilding {
			continue
		}
		perSrc++
		out = append(out, e)
	}
	return out
}

// funcKey identifies one function definition.
//
// The receiver is part of the key because a method name is only unique per
// type, not per file. `func (formBinding) Name()` and
// `func (formPostBinding) Name()` sit side by side in one gin source file, and
// Rust's `fn new()` appears in every `impl` block; keying on file and name
// alone made them collide, so a single type silently absorbed every
// same-named method in the file and its siblings showed none.
//
// C cannot produce this collision - it has neither methods nor overloading -
// which is why the bug only surfaced once a second language existed.
func funcKey(f *RawFunction) string {
	return f.SourceFile + "::" + f.Receiver + "#" + f.Name
}
