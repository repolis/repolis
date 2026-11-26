package analyzer

import (
	"cmp"
	"slices"
	"sort"
	"strings"

	"github.com/repolis/repolis/backend/internal/models"
)

// The largest component reported as a finding rather than a tangled region:
// up to this size a cycle is verifiable by hand against the source.
const maxReliableCycle = 12

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
	// Deterministic whatever order the maps upstream iterated in.
	for i := range adj {
		sort.Ints(adj[i])
	}

	components := stronglyConnectedComponents(adj)

	out := make([]models.Cycle, 0, len(components))
	for _, comp := range components {
		members := make([]string, 0, len(comp))
		namespaces := map[string]bool{}
		for _, n := range comp {
			members = append(members, buildings[n].ID)
			namespaces[buildings[n].Namespace] = true
		}
		out = append(out, models.Cycle{
			Size:       len(comp),
			Members:    members,
			Namespaces: len(namespaces),
			Reliable:   len(comp) <= maxReliableCycle,
		})
	}

	// Reliable cross-module cycles are the actionable findings, then the
	// largest; the first member breaks ties so the order is stable.
	actionable := func(c models.Cycle) bool { return c.Reliable && c.CrossModule() }
	slices.SortFunc(out, func(a, b models.Cycle) int {
		return cmp.Or(
			boolDesc(actionable(a), actionable(b)),
			boolDesc(a.CrossModule(), b.CrossModule()),
			cmp.Compare(b.Size, a.Size),
			strings.Compare(a.Members[0], b.Members[0]),
		)
	})
	for i := range out {
		out[i].ID = i + 1
	}
	return out
}

// Iterative on purpose: recursion depth would be bounded by the graph's
// longest path, and a blown stack here takes the whole request down.
func stronglyConnectedComponents(adj [][]int) [][]int {
	const unvisited = -1
	discovered := make([]int, len(adj))
	lowLink := make([]int, len(adj))
	onStack := make([]bool, len(adj))
	for i := range discovered {
		discovered[i] = unvisited
	}
	var stack []int
	counter := 0

	type frame struct {
		node     int
		nextEdge int
	}

	var components [][]int

	for root := range adj {
		if discovered[root] != unvisited {
			continue
		}
		work := []frame{{node: root}}

		for len(work) > 0 {
			f := &work[len(work)-1]
			node := f.node

			if f.nextEdge == 0 {
				discovered[node] = counter
				lowLink[node] = counter
				counter++
				stack = append(stack, node)
				onStack[node] = true
			}

			if f.nextEdge < len(adj[node]) {
				next := adj[node][f.nextEdge]
				f.nextEdge++
				switch {
				case discovered[next] == unvisited:
					work = append(work, frame{node: next})
				case onStack[next]:
					if discovered[next] < lowLink[node] {
						lowLink[node] = discovered[next]
					}
				}
				continue
			}

			if lowLink[node] == discovered[node] {
				var comp []int
				for {
					top := stack[len(stack)-1]
					stack = stack[:len(stack)-1]
					onStack[top] = false
					comp = append(comp, top)
					if top == node {
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
				parent := work[len(work)-1].node
				if lowLink[node] < lowLink[parent] {
					lowLink[parent] = lowLink[node]
				}
			}
		}
	}
	return components
}

// members. An edge leaving the component is not part of the cycle.
func MarkCycles(buildings []models.Building, edges []models.DependencyEdge, cycles []models.Cycle) {
	// Only findings are drawn: a 68-building tangle would bury the
	// two-building cycles that are verifiable and fixable.
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

// boolDesc orders true before false.
func boolDesc(a, b bool) int {
	switch {
	case a == b:
		return 0
	case a:
		return -1
	}
	return 1
}
