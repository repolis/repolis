package analyzer

import (
	"sort"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
)

// Visibility ranks. A symbol the caller can actually reach outranks an
// identically named one it cannot.
const (
	visOwn       = 4 // defined in the caller's own namespace
	visImported  = 3 // named by an import in this file
	visReachable = 2 // reachable through the import graph (C's include closure)
	visSameDir   = 1 // neither, but sitting next door
	visAnywhere  = 0 // last resort
	maxReachable = 400
)

// scopes is the resolved import graph: which namespaces each file can see,
// and what local aliases stand for.
//
// The first version resolved a call by bare name across the whole repository,
// preferring the same file and then the same directory. That is adequate for C,
// which has one global namespace, and wrong for everything else: `layout::new`
// and `camera::new` are different functions and a bare-name lookup cannot tell
// them apart. Resolving imports is what makes more than one language possible.
type scopes struct {
	sep string
	// nsByFile maps a file to the namespace its symbols belong to.
	nsByFile map[string]string
	// visible maps a file to the namespaces it can see, with a rank.
	visible map[string]map[string]int
	// aliases maps a file's local alias to the path it stands for.
	aliases map[string]map[string]string
	// moduleScoped records, per file, whether a bare name is confined to the
	// file's own module plus its explicit imports.
	moduleScoped map[string]bool
	// allNS is every namespace that actually defines something.
	allNS map[string]bool
	// bySuffix indexes namespaces by their final segment, for matching an
	// import path against a namespace without scanning them all.
	bySuffix map[string][]string
}

// lastSegment takes the final component of a path, using whichever separator
// appears last.
//
// Checking the language separator first was wrong for Go: the separator is "."
// and an import path is "github.com/owner/repo/internal/x", so the last dot is
// inside the host name and the "segment" came back as
// "com/owner/repo/internal/x".
func lastSegment(p, sep string) string {
	iSlash := strings.LastIndex(p, "/")
	iSep := -1
	if sep != "" && sep != "/" {
		iSep = strings.LastIndex(p, sep)
	}
	if iSep > iSlash {
		return p[iSep+len(sep):]
	}
	if iSlash >= 0 {
		return p[iSlash+1:]
	}
	return p
}

func buildScopes(raw *RawExtraction) *scopes {
	sc := &scopes{
		nsByFile:     make(map[string]string, len(raw.Files)),
		visible:      make(map[string]map[string]int, len(raw.Files)),
		aliases:      make(map[string]map[string]string),
		moduleScoped: make(map[string]bool),
		allNS:        make(map[string]bool),
		bySuffix:     make(map[string][]string),
	}

	// A single repository can hold more than one language. The separator only
	// matters for splitting qualified references, and mixing is rare enough
	// that the dominant language's separator is the right default.
	counts := map[string]int{}
	for _, f := range raw.Files {
		counts[f.Language]++
	}
	best, bestN := "", 0
	for name, n := range counts {
		if n > bestN || (n == bestN && name < best) {
			best, bestN = name, n
		}
	}
	for _, l := range lang.All() {
		if l.Name() == best {
			sc.sep = l.Separator()
		}
	}

	for _, f := range raw.Files {
		sc.nsByFile[f.Path] = f.Namespace
		if !sc.allNS[f.Namespace] {
			sc.allNS[f.Namespace] = true
			seg := lastSegment(f.Namespace, sc.sep)
			sc.bySuffix[seg] = append(sc.bySuffix[seg], f.Namespace)
		}
	}

	// Direct imports, plus the graph they induce.
	edges := make(map[string][]string, len(raw.Files))
	for _, f := range raw.Files {
		l := lang.ForFile(f.Path)
		if l == nil {
			continue
		}
		sc.moduleScoped[f.Path] = l.ModuleScoped()
		vis := map[string]int{f.Namespace: visOwn}

		for _, imp := range f.Imports {
			targets := l.ImportTargets(imp, f.Namespace)
			if len(targets) == 0 {
				continue
			}

			// An alias stands for the full path as written, not for whichever
			// prefix of it happens to be a module. `use a::b::area as alias`
			// has to remember "area", or a later bare call to `alias` has no
			// symbol name to look up.
			if imp.Alias != "" {
				if sc.aliases[f.Path] == nil {
					sc.aliases[f.Path] = map[string]string{}
				}
				sc.aliases[f.Path][imp.Alias] = targets[0]
			}

			for _, target := range targets {
				matches := sc.match(target)
				if len(matches) == 0 {
					continue
				}
				for _, m := range matches {
					if vis[m] < visImported {
						vis[m] = visImported
					}
					edges[f.Namespace] = append(edges[f.Namespace], m)
				}
				break // first target that matched a real namespace wins
			}
			// An import matching nothing is an external dependency; it simply
			// contributes no visibility.
		}
		sc.visible[f.Path] = vis
	}

	// Transitive reach. A C file routinely calls a function whose header it
	// picks up through another header, so stopping at direct includes would
	// lose real edges.
	for _, f := range raw.Files {
		vis := sc.visible[f.Path]
		if vis == nil {
			continue
		}
		queue := make([]string, 0, len(vis))
		for ns, rank := range vis {
			if rank >= visImported {
				queue = append(queue, ns)
			}
		}
		sort.Strings(queue)
		for i := 0; i < len(queue) && len(vis) < maxReachable; i++ {
			for _, next := range edges[queue[i]] {
				if _, seen := vis[next]; seen {
					continue
				}
				vis[next] = visReachable
				queue = append(queue, next)
				if len(vis) >= maxReachable {
					break
				}
			}
		}
	}

	return sc
}

// match finds the namespaces an import path refers to: exact first, then by
// path suffix, which is what turns `#include "git2/oid.h"` into
// `include/git2/oid.h` and leaves external crates unmatched.
func (sc *scopes) match(target string) []string {
	if target == "" {
		return nil
	}
	if sc.allNS[target] {
		return []string{target}
	}
	seg := lastSegment(target, sc.sep)
	var out []string
	for _, ns := range sc.bySuffix[seg] {
		switch {
		case ns == target,
			// The import names less than the namespace: `#include "oid.h"`
			// against `include/git2/oid.h`.
			strings.HasSuffix(ns, "/"+target),
			sc.sep != "" && strings.HasSuffix(ns, sc.sep+target),
			// The import names more than the namespace: Go writes the whole
			// module path where the namespace is only the directory.
			strings.HasSuffix(target, "/"+ns):
			out = append(out, ns)
		}
	}
	sort.Strings(out)
	return out
}

// rank scores how visible a namespace is from a file.
func (sc *scopes) rank(fromFile, ns string) int {
	if v, ok := sc.visible[fromFile]; ok {
		if r, ok := v[ns]; ok {
			return r
		}
	}
	// Same directory is a weak signal, but a real one in flat C projects.
	if own, ok := sc.nsByFile[fromFile]; ok {
		if dirOf(own) == dirOf(ns) {
			return visSameDir
		}
	}
	return visAnywhere
}

// expand turns a written qualifier into candidate namespaces, best first.
//
// The final segment of a path is often a type rather than a module
// (`Lot::area` where Lot is `crate::layout::Item`), so the parent path is
// offered too and the caller takes whichever resolves.
func (sc *scopes) expand(qualifier, fromFile string) []string {
	if qualifier == "" {
		return nil
	}
	q := qualifier

	// A leading alias stands for a longer path.
	if aliases, ok := sc.aliases[fromFile]; ok {
		head, rest := q, ""
		if sc.sep != "" {
			if i := strings.Index(q, sc.sep); i != -1 {
				head, rest = q[:i], q[i+len(sc.sep):]
			}
		}
		if full, ok := aliases[head]; ok {
			q = full
			if rest != "" {
				q += sc.sep + rest
			}
		}
	}

	// `self::` / `super::` were already expanded at import time; a bare
	// relative qualifier is resolved against the caller's own namespace.
	if own, ok := sc.nsByFile[fromFile]; ok && sc.sep != "" {
		if !strings.Contains(q, sc.sep) && !sc.allNS[q] {
			if nested := own + sc.sep + q; sc.allNS[nested] {
				return []string{nested, own}
			}
		}
	}

	out := make([]string, 0, 4)
	out = append(out, sc.match(q)...)
	if sc.sep != "" {
		if i := strings.LastIndex(q, sc.sep); i > 0 {
			out = append(out, sc.match(q[:i])...)
		}
	}
	return out
}

// IsModuleScoped reports whether bare names in this file are confined to its
// own module plus explicit imports.
func (sc *scopes) IsModuleScoped(fromFile string) bool { return sc.moduleScoped[fromFile] }

// OwnNamespace is the namespace a file's own symbols live in.
func (sc *scopes) OwnNamespace(fromFile string) string { return sc.nsByFile[fromFile] }

// aliasTarget resolves a local alias to the real symbol name and the
// namespaces to look for it in.
//
// `use crate::shapes::circle::area as circle_area` followed by a bare
// `circle_area(1.0)` is the case this exists for: the call site never mentions
// either the module or the real name, so without the alias table the edge is
// simply lost - or worse, matched against an unrelated `area` elsewhere.
func (sc *scopes) aliasTarget(name, fromFile string) (string, []string) {
	aliases := sc.aliases[fromFile]
	if aliases == nil {
		return "", nil
	}
	full, ok := aliases[name]
	if !ok {
		return "", nil
	}
	real := lastSegment(full, sc.sep)
	parent := ""
	if sc.sep != "" {
		if i := strings.LastIndex(full, sc.sep); i > 0 {
			parent = full[:i]
		}
	}
	if parent == "" {
		return real, nil
	}
	return real, sc.match(parent)
}

// qualifierType returns the trailing segment of a qualifier, which for a
// method call such as `Camera::new` is the receiver's type name.
func (sc *scopes) qualifierType(qualifier, fromFile string) string {
	if qualifier == "" {
		return ""
	}
	q := qualifier
	if aliases, ok := sc.aliases[fromFile]; ok {
		if full, ok := aliases[q]; ok {
			q = full
		}
	}
	return lastSegment(q, sc.sep)
}
