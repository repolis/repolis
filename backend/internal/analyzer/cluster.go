package analyzer

import (
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

// Cluster is a district before it has been named.
type Cluster struct {
	Buildings []string // building IDs
	TopDirs   []string
	TopNames  []string
	// Codebase-wide symbol prefix ("git_"), stripped before the model sees
	// the symbols: with it, every libgit2 district was named "Git Something".
	SymbolPrefix string
	// Noise holds the words every district here shares.
	Noise []string
	// Language names the code being described, for the prompt.
	Language string
}

// DistinctDirs returns the directory segments that separate this cluster from
// the rest, dropping path components every cluster shares.
func (c Cluster) DistinctDirs(common []string) []string {
	out := make([]string, 0, len(c.TopDirs))
	for _, d := range c.TopDirs {
		parts := strings.Split(d, "/")
		kept := parts[:0]
		for _, p := range parts {
			shared := false
			for _, cm := range common {
				if p == cm {
					shared = true
					break
				}
			}
			if !shared {
				kept = append(kept, p)
			}
		}
		if len(kept) > 0 {
			out = append(out, strings.Join(kept, "/"))
		} else {
			out = append(out, d)
		}
	}
	return out
}

// SplitIdentifier breaks an identifier into lower-case words.
func SplitIdentifier(name string) []string {
	var words []string
	var cur []rune
	flush := func() {
		if len(cur) > 0 {
			words = append(words, strings.ToLower(string(cur)))
			cur = cur[:0]
		}
	}
	runes := []rune(name)
	for i, r := range runes {
		switch {
		case r == '_' || r == '-' || r == ' ':
			flush()
		case r >= 'A' && r <= 'Z':
			// Only at a lower->upper transition, so acronyms stay together.
			if i > 0 && runes[i-1] >= 'a' && runes[i-1] <= 'z' {
				flush()
			}
			cur = append(cur, r)
		default:
			cur = append(cur, r)
		}
	}
	flush()
	return words
}

// CommonSymbolTokens finds words appearing in most clusters' symbols, which
// therefore identify none of them. Measured per cluster, not as a global
// prefix: libgit2's test suite dilutes "git_" below any global threshold, yet
// "git" is still in every district and swamped every name the model produced.
func CommonSymbolTokens(clusters []Cluster) []string {
	if len(clusters) < 3 {
		return nil
	}
	counts := map[string]int{}
	for _, c := range clusters {
		seen := map[string]bool{}
		for _, n := range c.TopNames {
			for _, w := range SplitIdentifier(n) {
				if len(w) < 2 || seen[w] {
					continue
				}
				seen[w] = true
				counts[w]++
			}
		}
	}
	threshold := (len(clusters) * 3) / 5
	if threshold < 2 {
		threshold = 2
	}
	var out []string
	for w, n := range counts {
		if n >= threshold {
			out = append(out, w)
		}
	}
	sort.Strings(out)
	return out
}

// CommonDirSegments finds path components present in most clusters, which
// therefore say nothing about any individual one.
func CommonDirSegments(clusters []Cluster) []string {
	counts := map[string]int{}
	for _, c := range clusters {
		seen := map[string]bool{}
		for _, d := range c.TopDirs {
			for _, p := range strings.Split(d, "/") {
				if !seen[p] {
					seen[p] = true
					counts[p]++
				}
			}
		}
	}
	threshold := (len(clusters) * 2) / 3
	if threshold < 2 {
		return nil
	}
	var out []string
	for p, n := range counts {
		if n >= threshold {
			out = append(out, p)
		}
	}
	sort.Strings(out)
	return out
}

type clNode struct {
	id  string
	dir string
}

type clEdge struct {
	to int
	w  float64
}

// ClusterBuildings partitions buildings into districts by weighted label
// propagation over the call graph plus directory affinity. Deterministic on
// purpose: community detection is a graph problem with a correct answer, and
// asking a 4B model to emit the partition collapsed sqlite into one
// 221-building district.
func ClusterBuildings(buildings []models.Building, edges []models.DependencyEdge) []Cluster {
	if len(buildings) == 0 {
		return nil
	}

	nodes := make([]clNode, len(buildings))
	index := make(map[string]int, len(buildings))
	for i, b := range buildings {
		nodes[i] = clNode{id: b.ID, dir: b.Dir}
		index[b.ID] = i
	}

	adj := make([][]clEdge, len(nodes))
	addEdge := func(a, b int, w float64) {
		if a == b {
			return
		}
		adj[a] = append(adj[a], clEdge{b, w})
		adj[b] = append(adj[b], clEdge{a, w})
	}

	for _, e := range edges {
		a, okA := index[e.Source]
		b, okB := index[e.Target]
		if !okA || !okB {
			continue
		}
		w := float64(e.Weight)
		if e.Kind == "type" {
			w *= 1.5 // composition is a stronger cohesion signal than a call
		}
		addEdge(a, b, w)
	}

	// Sparse small-world rather than a clique: linking each building to the
	// next few in its directory propagates labels fast at O(n) edges.
	byDir := make(map[string][]int)
	for i, n := range nodes {
		byDir[n.dir] = append(byDir[n.dir], i)
	}
	dirKeys := make([]string, 0, len(byDir))
	for d := range byDir {
		dirKeys = append(dirKeys, d)
	}
	sort.Strings(dirKeys)
	for _, d := range dirKeys {
		m := byDir[d]
		sort.Slice(m, func(i, j int) bool { return nodes[m[i]].id < nodes[m[j]].id })
		for i := range m {
			for _, step := range []int{1, 2, 4, 8} {
				if i+step < len(m) {
					addEdge(m[i], m[i+step], dirAffinity)
				}
			}
		}
	}

	// Initial labels: one per directory.
	dirLabel := make(map[string]int, len(dirKeys))
	for i, d := range dirKeys {
		dirLabel[d] = i
	}
	labels := make([]int, len(nodes))
	for i, n := range nodes {
		labels[i] = dirLabel[n.dir]
	}

	order := make([]int, len(nodes))
	for i := range order {
		order[i] = i
	}
	sort.Slice(order, func(i, j int) bool { return nodes[order[i]].id < nodes[order[j]].id })

	sizes := make(map[int]int)
	for _, l := range labels {
		sizes[l]++
	}

	// The sqrt(size) penalty stops large communities swallowing their
	// neighbours; plain propagation collapses a dense graph into one.
	for iter := 0; iter < lpIterations; iter++ {
		changed := 0
		for _, i := range order {
			if len(adj[i]) == 0 {
				continue
			}
			score := make(map[int]float64)
			for _, e := range adj[i] {
				score[labels[e.to]] += e.w
			}
			bestLabel, bestScore := labels[i], -1.0
			for l, s := range score {
				sz := sizes[l]
				if sz < 1 {
					sz = 1
				}
				weighted := s / sqrtInt(sz)
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

	groups := regroup(nodes, labels)
	groups = mergeSmall(groups, nodes, index, adj)
	groups = splitLarge(groups, nodes)
	groups = capCount(groups, nodes, index, adj)

	return finalize(groups, nodes, buildings, index)
}

func sqrtInt(n int) float64 {
	f := float64(n)
	if f <= 1 {
		return 1
	}
	x := f
	for i := 0; i < 12; i++ {
		x = 0.5 * (x + f/x)
	}
	return x
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
		sort.Slice(g, func(i, j int) bool { return nodes[g[i]].id < nodes[g[j]].id })
		out = append(out, g)
	}
	return out
}

// strongestNeighbour returns the index of the group most connected to g.
func strongestNeighbour(g []int, groupOf []int, adj [][]clEdge, self int) int {
	score := make(map[int]float64)
	for _, n := range g {
		for _, e := range adj[n] {
			gi := groupOf[e.to]
			if gi != self {
				score[gi] += e.w
			}
		}
	}
	best, bestScore := -1, -1.0
	for gi, s := range score {
		if s > bestScore || (s == bestScore && gi < best) {
			best, bestScore = gi, s
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

func mergeSmall(groups [][]int, nodes []clNode, _ map[string]int, adj [][]clEdge) [][]int {
	for {
		groupOf := groupIndex(groups, len(nodes))
		victim := -1
		for gi, g := range groups {
			if len(g) > 0 && len(g) < minDistrictSize {
				victim = gi
				break
			}
		}
		if victim == -1 {
			return compact(groups)
		}
		target := strongestNeighbour(groups[victim], groupOf, adj, victim)
		if target == -1 {
			// Wholly disconnected: fold into the largest group instead.
			target = 0
			for gi, g := range groups {
				if gi != victim && len(g) > len(groups[target]) {
					target = gi
				}
			}
			if target == victim {
				return compact(groups)
			}
		}
		groups[target] = append(groups[target], groups[victim]...)
		sort.Slice(groups[target], func(i, j int) bool {
			return nodes[groups[target][i]].id < nodes[groups[target][j]].id
		})
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

		var current []int
		for _, d := range dirs {
			part := byDir[d]
			for len(part) > 0 {
				room := maxDistrictSize - len(current)
				if room <= 0 {
					out = append(out, current)
					current = nil
					room = maxDistrictSize
				}
				take := len(part)
				if take > room {
					take = room
				}
				current = append(current, part[:take]...)
				part = part[take:]
			}
		}
		if len(current) > 0 {
			out = append(out, current)
		}
	}
	return out
}

func capCount(groups [][]int, nodes []clNode, _ map[string]int, adj [][]clEdge) [][]int {
	groups = compact(groups)
	for len(groups) > maxDistricts {
		groupOf := groupIndex(groups, len(nodes))
		smallest := 0
		for gi := range groups {
			if len(groups[gi]) < len(groups[smallest]) {
				smallest = gi
			}
		}
		target := strongestNeighbour(groups[smallest], groupOf, adj, smallest)
		if target == -1 || len(groups[target])+len(groups[smallest]) > maxDistrictSize*2 {
			// Next-smallest, so we always make progress.
			target = -1
			for gi := range groups {
				if gi == smallest {
					continue
				}
				if target == -1 || len(groups[gi]) < len(groups[target]) {
					target = gi
				}
			}
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

func finalize(groups [][]int, nodes []clNode, buildings []models.Building, index map[string]int) []Cluster {
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
			b := buildings[index[id]]
			names = append(names, nb{b.Name, b.NumMethods*3 + b.NumFields})
		}
		sort.Slice(names, func(i, j int) bool {
			if names[i].w != names[j].w {
				return names[i].w > names[j].w
			}
			return names[i].name < names[j].name
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
		sort.Slice(dirs, func(i, j int) bool {
			if dirs[i].n != dirs[j].n {
				return dirs[i].n > dirs[j].n
			}
			return dirs[i].d < dirs[j].d
		})
		for i := 0; i < len(dirs) && i < 3; i++ {
			c.TopDirs = append(c.TopDirs, dirs[i].d)
		}
		out = append(out, c)
	}

	sort.Slice(out, func(i, j int) bool {
		if len(out[i].Buildings) != len(out[j].Buildings) {
			return len(out[i].Buildings) > len(out[j].Buildings)
		}
		return out[i].Buildings[0] < out[j].Buildings[0]
	})
	return out
}
