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

// scopes is the resolved import graph: which namespaces each file can see, and
// what its local aliases stand for. Resolving imports is what makes more than
// one language possible: bare-name lookup across the repo works for C's single
// global namespace, but cannot tell `layout::new` from `camera::new`.
type scopes struct {
	sep          string
	nsByFile     map[string]string            // file -> namespace its symbols belong to
	visible      map[string]map[string]int    // file -> namespaces it can see, ranked
	aliases      map[string]map[string]string // file -> local alias -> path
	moduleScoped map[string]bool              // file -> are bare names confined to its module?
	allNS        map[string]bool              // namespaces that define something
	bySuffix     map[string][]string          // final segment -> namespaces, for import matching
}

// lastSegment takes the final path component, using whichever separator
// appears last: Go's separator is "." but its import paths contain a host
// name, so preferring the separator yields "com/owner/repo/internal/x".
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

	// A repo can hold several languages, but the separator only splits
	// qualified references, so the dominant language's is a fine default.
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

			// The alias stands for the full path, not the module prefix:
			// `use a::b::area as alias` must remember "area".
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
			// An import matching nothing is external: no visibility.
		}
		sc.visible[f.Path] = vis
	}

	// Transitive reach: a C file routinely calls through a header it picks up
	// via another header.
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

// match finds the namespaces an import refers to: exact, then by path suffix,
// which maps `#include "git2/oid.h"` onto `include/git2/oid.h` and leaves
// external crates unmatched.
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
			// Import names less than the namespace.
			strings.HasSuffix(ns, "/"+target),
			sc.sep != "" && strings.HasSuffix(ns, sc.sep+target),
			// Import names more than it: Go writes the whole module path.
			strings.HasSuffix(target, "/"+ns):
			out = append(out, ns)
		}
	}
	sort.Strings(out)
	return out
}

func (sc *scopes) rank(fromFile, ns string) int {
	if v, ok := sc.visible[fromFile]; ok {
		if r, ok := v[ns]; ok {
			return r
		}
	}
	// Weak, but real in flat C projects.
	if own, ok := sc.nsByFile[fromFile]; ok {
		if dirOf(own) == dirOf(ns) {
			return visSameDir
		}
	}
	return visAnywhere
}

// expand turns a written qualifier into candidate namespaces, best first. The
// final segment is often a type rather than a module (`Lot::area`), so the
// parent path is offered too and the caller takes whichever resolves.
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

	// `self::`/`super::` are expanded at import time; a bare relative
	// qualifier resolves against the caller's own namespace.
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

func (sc *scopes) IsModuleScoped(fromFile string) bool { return sc.moduleScoped[fromFile] }

func (sc *scopes) OwnNamespace(fromFile string) string { return sc.nsByFile[fromFile] }

// aliasTarget resolves a local alias to the real symbol name and where to look
// for it. A bare `circle_area(1.0)` under `use ...::area as circle_area` names
// neither the module nor the real name, so the edge is otherwise lost.
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

// qualifierType returns a qualifier's trailing segment, which for `Camera::new`
// is the receiver's type name.
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
