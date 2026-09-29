package analyzer

import (
	"sort"

	"github.com/repolis/repolis/backend/internal/models"
)

// maxReliableCycle is the largest component reported as a finding rather than
// as a tangled region. Chosen because components up to this size were
// verifiable by hand against the source; larger ones were not.
const maxReliableCycle = 12

// FindCycles labels every strongly connected component of the call graph that
// contains more than one building, using Tarjan's algorithm.
//
// A dependency cycle is the clearest thing a city can show that a file tree
// cannot: it is a property of the graph, invisible in any directory listing,
// and an IDE will only reveal it one hop at a time. Everything needed is
// already in the edge set, so this costs one linear pass.
//
// The traversal is iterative on purpose. A recursive Tarjan over libgit2's
// ~1300 buildings is fine, but the depth is bounded by the longest path in the
// graph rather than by anything we control, and a blown stack in the analysis
// pass would take the whole request down.
func FindCycles(buildings []models.Building, edges []models.DependencyEdge) []models.Cycle {
	index := make(map[string]int, len(buildings))
	for i, b := range buildings {
		index[b.ID] = i
	}

	adj := make([][]int, len(buildings))
	for _, e := range edges {
		from, okF := index[e.Source]
		to, okT := index[e.Target]
		if !okF || !okT || from == to {
			continue
		}
		adj[from] = append(adj[from], to)
	}
	// Deterministic output regardless of map iteration order upstream.
	for i := range adj {
		sort.Ints(adj[i])
	}

	const unvisited = -1
	disc := make([]int, len(buildings))
	low := make([]int, len(buildings))
	onStack := make([]bool, len(buildings))
	for i := range disc {
		disc[i] = unvisited
	}
	var stack []int
	counter := 0

	type frame struct {
		v    int
		next int
	}

	var components [][]int

	for root := range buildings {
		if disc[root] != unvisited {
			continue
		}
		work := []frame{{v: root}}

		for len(work) > 0 {
			f := &work[len(work)-1]
			v := f.v

			if f.next == 0 {
				disc[v] = counter
				low[v] = counter
				counter++
				stack = append(stack, v)
				onStack[v] = true
			}

			if f.next < len(adj[v]) {
				w := adj[v][f.next]
				f.next++
				switch {
				case disc[w] == unvisited:
					work = append(work, frame{v: w})
				case onStack[w]:
					if disc[w] < low[v] {
						low[v] = disc[w]
					}
				}
				continue
			}

			// v is finished: close its component if it is a root.
			if low[v] == disc[v] {
				var comp []int
				for {
					w := stack[len(stack)-1]
					stack = stack[:len(stack)-1]
					onStack[w] = false
					comp = append(comp, w)
					if w == v {
						break
					}
				}
				if len(comp) > 1 {
					sort.Ints(comp)
					components = append(components, comp)
				}
			}

			work = work[:len(work)-1]
			if len(work) > 0 {
				parent := work[len(work)-1].v
				if low[v] < low[parent] {
					low[parent] = low[v]
				}
			}
		}
	}

	// Largest first: those are the ones worth looking at.
	sort.Slice(components, func(i, j int) bool {
		if len(components[i]) != len(components[j]) {
			return len(components[i]) > len(components[j])
		}
		return components[i][0] < components[j][0]
	})

	out := make([]models.Cycle, 0, len(components))
	for i, comp := range components {
		members := make([]string, 0, len(comp))
		namespaces := map[string]bool{}
		for _, n := range comp {
			members = append(members, buildings[n].ID)
			namespaces[buildings[n].Namespace] = true
		}
		out = append(out, models.Cycle{
			ID:         i + 1,
			Size:       len(comp),
			Members:    members,
			Namespaces: len(namespaces),
			Reliable:   len(comp) <= maxReliableCycle,
		})
	}

	// Reliable cross-module cycles first: those are the actionable findings.
	// Then everything else by size.
	sort.SliceStable(out, func(i, j int) bool {
		ai := out[i].Reliable && out[i].CrossModule()
		aj := out[j].Reliable && out[j].CrossModule()
		if ai != aj {
			return ai
		}
		if out[i].CrossModule() != out[j].CrossModule() {
			return out[i].CrossModule()
		}
		return out[i].Size > out[j].Size
	})
	for i := range out {
		out[i].ID = i + 1
	}
	return out
}

// MarkCycles writes the component id onto each member building and flags every
// edge that runs between two members of the same component. Only those edges
// are part of a cycle; an edge leaving the component is not.
func MarkCycles(buildings []models.Building, edges []models.DependencyEdge, cycles []models.Cycle) {
	// Only cycles we are prepared to call findings are marked for display.
	// Drawing a 68-building tangle would bury the two-building cycles that
	// are both verifiable and fixable.
	of := make(map[string]int, len(buildings))
	for _, c := range cycles {
		if !c.Reliable || !c.CrossModule() {
			continue
		}
		for _, id := range c.Members {
			of[id] = c.ID
		}
	}
	for i := range buildings {
		buildings[i].CycleID = of[buildings[i].ID]
	}
	for i := range edges {
		src := of[edges[i].Source]
		edges[i].InCycle = src != 0 && src == of[edges[i].Target]
	}
}
