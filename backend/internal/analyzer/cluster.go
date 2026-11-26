package analyzer

import (
	"cmp"
	"math"
	"slices"
	"sort"
	"strings"

	"github.com/repolis/repolis/backend/internal/models"
)

const (
	minDistrictSize = 3
	maxDistrictSize = 60
	maxDistricts    = 16
	dirAffinity     = 2.5
	lpIterations    = 12
)

type Cluster struct {
	Buildings []string // building IDs
	TopDirs   []string
	TopNames  []string
	// Codebase-wide symbol prefix ("git_"), stripped before the model sees
	// the symbols: with it, every libgit2 district was named "Git Something".
	SymbolPrefix string
	Noise        []string
	Language     string
}

type clNode struct {
	id  string
	dir string
}

type clEdge struct {
	to     int
	weight float64
}

// Deterministic on purpose: asking a 4B model to emit the partition collapsed
// sqlite into one 221-building district.
func ClusterBuildings(buildings []models.Building, edges []models.DependencyEdge) []Cluster {
	if len(buildings) == 0 {
		return nil
	}

	nodes, adj := buildClusterGraph(buildings, edges)

	byDir := make(map[string][]int)
	for i, n := range nodes {
		byDir[n.dir] = append(byDir[n.dir], i)
	}
	dirs := make([]string, 0, len(byDir))
	for d := range byDir {
		dirs = append(dirs, d)
	}
	sort.Strings(dirs)

	linkDirectoryNeighbours(adj, nodes, byDir, dirs)
	labels := labelsByDirectory(nodes, dirs)
	propagateLabels(labels, nodes, adj)

	groups := regroup(nodes, labels)
	groups = mergeSmall(groups, nodes, adj)
	groups = splitLarge(groups, nodes)
	groups = capCount(groups, nodes, adj)

	return finalize(groups, nodes, buildings)
}

func addClusterEdge(adj [][]clEdge, a, b int, weight float64) {
	if a == b {
		return
	}
	adj[a] = append(adj[a], clEdge{b, weight})
	adj[b] = append(adj[b], clEdge{a, weight})
}

func buildClusterGraph(buildings []models.Building, edges []models.DependencyEdge) ([]clNode, [][]clEdge) {
	nodes := make([]clNode, len(buildings))
	index := make(map[string]int, len(buildings))
	for i, b := range buildings {
		nodes[i] = clNode{id: b.ID, dir: b.Dir}
		index[b.ID] = i
	}

	adj := make([][]clEdge, len(nodes))
	for _, e := range edges {
		a, okA := index[e.Source]
		b, okB := index[e.Target]
		if !okA || !okB {
			continue
		}
		weight := float64(e.Weight)
		if e.Kind == "type" {
			weight *= 1.5 // composition is a stronger cohesion signal than a call
		}
		addClusterEdge(adj, a, b, weight)
	}
	return nodes, adj
}

// Sparse small-world rather than a clique: linking each building to the
// next few in its directory propagates labels fast at O(n) edges.
func linkDirectoryNeighbours(adj [][]clEdge, nodes []clNode, byDir map[string][]int, dirs []string) {
	for _, d := range dirs {
		members := byDir[d]
		sortByID(members, nodes)
		for i := range members {
			for _, step := range []int{1, 2, 4, 8} {
				if i+step < len(members) {
					addClusterEdge(adj, members[i], members[i+step], dirAffinity)
				}
			}
		}
	}
}

func labelsByDirectory(nodes []clNode, dirs []string) []int {
	dirLabel := make(map[string]int, len(dirs))
	for i, d := range dirs {
		dirLabel[d] = i
	}
	labels := make([]int, len(nodes))
	for i, n := range nodes {
		labels[i] = dirLabel[n.dir]
	}
	return labels
}

// The sqrt(size) penalty stops large communities swallowing their
// neighbours; plain propagation collapses a dense graph into one.
func propagateLabels(labels []int, nodes []clNode, adj [][]clEdge) {
	order := make([]int, len(nodes))
	for i := range order {
		order[i] = i
	}
	sortByID(order, nodes)

	sizes := make(map[int]int)
	for _, l := range labels {
		sizes[l]++
	}

	for iter := 0; iter < lpIterations; iter++ {
		changed := 0
		for _, i := range order {
			if len(adj[i]) == 0 {
				continue
			}
			score := make(map[int]float64)
			for _, e := range adj[i] {
				score[labels[e.to]] += e.weight
			}
			bestLabel, bestScore := labels[i], -1.0
			for l, s := range score {
				weighted := s / math.Sqrt(float64(max(sizes[l], 1)))
				if weighted > bestScore || (weighted == bestScore && l < bestLabel) {
					bestLabel, bestScore = l, weighted
				}
			}
			if bestLabel != labels[i] {
				sizes[labels[i]]--
				sizes[bestLabel]++
				labels[i] = bestLabel
				changed++
			}
		}
		if changed == 0 {
			break
		}
	}
}

func sortByID(group []int, nodes []clNode) {
	sort.Slice(group, func(i, j int) bool { return nodes[group[i]].id < nodes[group[j]].id })
}

func regroup(nodes []clNode, labels []int) [][]int {
	byLabel := make(map[int][]int)
	for i := range nodes {
		byLabel[labels[i]] = append(byLabel[labels[i]], i)
	}
	keys := make([]int, 0, len(byLabel))
	for k := range byLabel {
		keys = append(keys, k)
	}
	sort.Ints(keys)
	out := make([][]int, 0, len(keys))
	for _, k := range keys {
		g := byLabel[k]
		sortByID(g, nodes)
		out = append(out, g)
	}
	return out
}

func strongestNeighbour(g []int, groupOf []int, adj [][]clEdge, self int) int {
	score := make(map[int]float64)
	for _, n := range g {
		for _, e := range adj[n] {
			other := groupOf[e.to]
			if other != self {
				score[other] += e.weight
			}
		}
	}
	best, bestScore := -1, -1.0
	for other, s := range score {
		if s > bestScore || (s == bestScore && other < best) {
			best, bestScore = other, s
		}
	}
	return best
}

func groupIndex(groups [][]int, total int) []int {
	groupOf := make([]int, total)
	for i := range groupOf {
		groupOf[i] = -1
	}
	for gi, g := range groups {
		for _, n := range g {
			groupOf[n] = gi
		}
	}
	return groupOf
}

func largestOther(groups [][]int, self int) int {
	largest := 0
	for i, g := range groups {
		if i != self && len(g) > len(groups[largest]) {
			largest = i
		}
	}
	return largest
}

func smallestOther(groups [][]int, self int) int {
	smallest := -1
	for i, g := range groups {
		if i == self {
			continue
		}
		if smallest == -1 || len(g) < len(groups[smallest]) {
			smallest = i
		}
	}
	return smallest
}

func mergeSmall(groups [][]int, nodes []clNode, adj [][]clEdge) [][]int {
	for {
		groupOf := groupIndex(groups, len(nodes))
		victim := -1
		for i, g := range groups {
			if len(g) > 0 && len(g) < minDistrictSize {
				victim = i
				break
			}
		}
		if victim == -1 {
			return compact(groups)
		}
		target := strongestNeighbour(groups[victim], groupOf, adj, victim)
		if target == -1 {
			// Wholly disconnected: fold into the largest group instead.
			target = largestOther(groups, victim)
			if target == victim {
				return compact(groups)
			}
		}
		groups[target] = append(groups[target], groups[victim]...)
		sortByID(groups[target], nodes)
		groups[victim] = nil
	}
}

func splitLarge(groups [][]int, nodes []clNode) [][]int {
	var out [][]int
	for _, g := range groups {
		if len(g) <= maxDistrictSize {
			out = append(out, g)
			continue
		}
		// By directory first: the most legible sub-structure available.
		byDir := make(map[string][]int)
		for _, n := range g {
			byDir[nodes[n].dir] = append(byDir[nodes[n].dir], n)
		}
		dirs := make([]string, 0, len(byDir))
		for d := range byDir {
			dirs = append(dirs, d)
		}
		sort.Strings(dirs)

		byDirOrder := make([]int, 0, len(g))
		for _, d := range dirs {
			byDirOrder = append(byDirOrder, byDir[d]...)
		}
		for chunk := range slices.Chunk(byDirOrder, maxDistrictSize) {
			out = append(out, chunk)
		}
	}
	return out
}

func capCount(groups [][]int, nodes []clNode, adj [][]clEdge) [][]int {
	groups = compact(groups)
	for len(groups) > maxDistricts {
		groupOf := groupIndex(groups, len(nodes))
		smallest := 0
		for i := range groups {
			if len(groups[i]) < len(groups[smallest]) {
				smallest = i
			}
		}
		target := strongestNeighbour(groups[smallest], groupOf, adj, smallest)
		if target == -1 || len(groups[target])+len(groups[smallest]) > maxDistrictSize*2 {
			// Next-smallest, so we always make progress.
			target = smallestOther(groups, smallest)
			if target == -1 {
				break
			}
		}
		groups[target] = append(groups[target], groups[smallest]...)
		groups[smallest] = nil
		groups = compact(groups)
	}
	return groups
}

func compact(groups [][]int) [][]int {
	out := make([][]int, 0, len(groups))
	for _, g := range groups {
		if len(g) > 0 {
			out = append(out, g)
		}
	}
	return out
}

func finalize(groups [][]int, nodes []clNode, buildings []models.Building) []Cluster {
	out := make([]Cluster, 0, len(groups))
	for _, g := range groups {
		c := Cluster{}
		dirCount := make(map[string]int)
		type nb struct {
			name string
			w    int
		}
		var names []nb
		for _, n := range g {
			id := nodes[n].id
			c.Buildings = append(c.Buildings, id)
			dirCount[nodes[n].dir]++
			b := buildings[n]
			names = append(names, nb{b.Name, b.NumMethods*3 + b.NumFields})
		}
		slices.SortFunc(names, func(a, b nb) int {
			return cmp.Or(cmp.Compare(b.w, a.w), strings.Compare(a.name, b.name))
		})
		for i := 0; i < len(names) && i < 12; i++ {
			c.TopNames = append(c.TopNames, names[i].name)
		}

		type dc struct {
			d string
			n int
		}
		var dirs []dc
		for d, n := range dirCount {
			dirs = append(dirs, dc{d, n})
		}
		slices.SortFunc(dirs, func(a, b dc) int {
			return cmp.Or(cmp.Compare(b.n, a.n), strings.Compare(a.d, b.d))
		})
		for i := 0; i < len(dirs) && i < 3; i++ {
			c.TopDirs = append(c.TopDirs, dirs[i].d)
		}
		out = append(out, c)
	}

	slices.SortFunc(out, func(a, b Cluster) int {
		return cmp.Or(cmp.Compare(len(b.Buildings), len(a.Buildings)), strings.Compare(a.Buildings[0], b.Buildings[0]))
	})
	return out
}
