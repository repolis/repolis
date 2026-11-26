package analyzer

import (
	"slices"
	"testing"

	"github.com/repolis/repolis/backend/internal/models"
)

func TestKeepStrongestBreaksTiesByKind(t *testing.T) {
	call := models.DependencyEdge{Source: "A", Target: "B", Weight: 2, Kind: "call"}
	typ := models.DependencyEdge{Source: "A", Target: "B", Weight: 2, Kind: "type"}

	for _, in := range [][]models.DependencyEdge{{call, typ}, {typ, call}} {
		got := keepStrongest(slices.Clone(in), 1)
		if len(got) != 1 || got[0].Kind != "call" {
			t.Errorf("keepStrongest(%v, 1) = %v; want the call edge whatever the input order", in, got)
		}
	}
}
