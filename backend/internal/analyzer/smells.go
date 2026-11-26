package analyzer

import (
	"sort"

	"github.com/repolis/repolis/backend/internal/models"
)

const (
	SmellGod   = "god"
	SmellBrain = "brain"
	SmellData  = "data"
)

const (
	godMinTotalComplexity = 40
	godMinFanOut          = 5
	godMinMethods         = 8
	brainMinComplexity    = 15
	dataMinFields         = 8
)

// Rough stand-ins for Lanza & Marinescu's detection strategies, which need
// metrics we do not collect. Floors keep small repos from flagging anything.
func markSmells(buildings []models.Building) {
	maxComplexity := p95(buildings, func(b models.Building) int { return b.MaxComplexity })
	sumComplexity := p95(buildings, func(b models.Building) int { return b.SumComplexity })
	fanOut := p95(buildings, func(b models.Building) int { return b.FanOut })

	for i := range buildings {
		b := &buildings[i]
		isGod := b.SumComplexity >= max(godMinTotalComplexity, sumComplexity) &&
			b.FanOut >= max(godMinFanOut, fanOut) &&
			b.NumMethods >= godMinMethods
		switch {
		case isGod:
			b.Smell = SmellGod
		case b.MaxComplexity >= max(brainMinComplexity, maxComplexity):
			b.Smell = SmellBrain
		case b.Kind == "type" && b.NumFields >= dataMinFields && b.NumMethods == 0:
			b.Smell = SmellData
		}
	}
}

// p95, not max: libgit2's worst function is 129 and nearly everything is under 15.
func p95(buildings []models.Building, metric func(models.Building) int) int {
	if len(buildings) == 0 {
		return 0
	}
	values := make([]int, len(buildings))
	for i, b := range buildings {
		values[i] = metric(b)
	}
	sort.Ints(values)
	return max(values[len(values)*95/100], 1)
}
